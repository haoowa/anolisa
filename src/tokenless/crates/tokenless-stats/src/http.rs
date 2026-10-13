//! Optional HTTP statistics queries protected by a single service credential.

use std::io::Read;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use actix_web::{App, HttpRequest, HttpResponse, HttpServer, http::header, web};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::reader::{ReadError, RecordQuery, StatsReader};

const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

/// Startup failures for the optional query service.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    /// The configured statistics source cannot be read.
    #[error(transparent)]
    Source(#[from] ReadError),
    /// Credential file access or the HTTP listener failed.
    #[error("statistics service I/O failed: {0}")]
    Io(#[from] std::io::Error),
    /// No usable service credential was configured.
    #[error("credential must be nonempty ASCII without whitespace, at most 4096 bytes")]
    Credential,
}

struct State {
    database: PathBuf,
    credential: blake3::Hash,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct DetailQuery {
    include_content: bool,
}

/// Run a standalone HTTP listener over an existing database.
///
/// The credential is loaded once; restart after rotating the file. Remote
/// deployments must protect the transport using a private network or TLS proxy.
/// # Errors
/// Returns credential, source-access, or listener errors. Never initializes the database.
pub fn serve(
    database: PathBuf,
    listen: SocketAddr,
    credential_file: &Path,
) -> Result<(), ServerError> {
    let credential = load_credential(credential_file)?;
    StatsReader::open(&database)?;
    let state = web::Data::new(State {
        database,
        credential,
    });
    actix_web::rt::System::new().block_on(async move {
        HttpServer::new(move || {
            App::new()
                .app_data(state.clone())
                .default_service(web::route().to(query))
        })
        .workers(2)
        .bind(listen)?
        .run()
        .await
    })?;
    Ok(())
}

fn load_credential(path: &Path) -> Result<blake3::Hash, ServerError> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(4097)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err(ServerError::Credential);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| ServerError::Credential)?;
    let token = text.trim_end_matches(['\r', '\n']);
    if token.is_empty() || token.len() > 4096 || !token.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(ServerError::Credential);
    }
    Ok(blake3::hash(token.as_bytes()))
}

fn authorized(request: &HttpRequest, state: &State) -> bool {
    if request.headers().get_all(header::AUTHORIZATION).count() != 1 {
        return false;
    }
    request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("Bearer"))
        // Hash equality is constant-time; the plaintext credential never enters logs or responses.
        .is_some_and(|(_, token)| blake3::hash(token.as_bytes()) == state.credential)
}

fn error(status: u16, code: &str) -> HttpResponse {
    let status = actix_web::http::StatusCode::from_u16(status)
        .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR);
    HttpResponse::build(status)
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(json!({"schema_version":1,"error":{"code":code}}))
}

async fn query(request: HttpRequest, state: web::Data<State>) -> HttpResponse {
    if !authorized(&request, &state) {
        let mut response = error(401, "unauthorized");
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            header::HeaderValue::from_static("Bearer"),
        );
        return response;
    }
    if request.method() != actix_web::http::Method::GET {
        return error(405, "method_not_allowed");
    }
    let database = state.database.clone();
    let result = if request.path() == "/v1/stats/records" {
        let Ok(filter) = web::Query::<RecordQuery>::from_query(request.query_string()) else {
            return error(400, "invalid_query");
        };
        web::block(move || {
            let page = StatsReader::open(&database)?.list(&filter)?;
            Ok::<_, ReadError>(Some(json!({"schema_version":1,"records":page.records,"next_before_id":page.next_before_id})))
        }).await
    } else if let Some(id) = request.path().strip_prefix("/v1/stats/records/") {
        let Ok(id) = id.parse::<i64>() else {
            return error(400, "invalid_query");
        };
        let Ok(filter) = web::Query::<DetailQuery>::from_query(request.query_string()) else {
            return error(400, "invalid_query");
        };
        web::block(move || {
            Ok::<_, ReadError>(
                StatsReader::open(&database)?
                    .get(id, filter.include_content)?
                    .map(|record| json!({"schema_version":1,"record":record})),
            )
        })
        .await
    } else {
        return error(404, "not_found");
    };
    match result {
        Ok(Ok(Some(value))) => response(value),
        Ok(Ok(None)) => error(404, "not_found"),
        Ok(Err(ReadError::InvalidQuery(_))) => error(400, "invalid_query"),
        Ok(Err(ReadError::InvalidRecord(_))) => error(500, "invalid_record"),
        Ok(Err(ReadError::Unsupported(_))) => error(503, "unsupported_layout"),
        _ => error(503, "database_unavailable"),
    }
}

fn response(value: Value) -> HttpResponse {
    match serde_json::to_vec(&value) {
        Ok(bytes) if bytes.len() <= MAX_RESPONSE_BYTES => HttpResponse::Ok()
            .insert_header((header::CONTENT_TYPE, "application/json"))
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .body(bytes),
        Ok(_) => error(413, "response_too_large"),
        Err(_) => error(500, "serialization_failed"),
    }
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
