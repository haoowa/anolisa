use super::*;
use crate::{OperationType, StatsRecord, StatsRecorder};
use actix_web::{http::StatusCode, test};

#[actix_web::test]
async fn authentication_query_and_failure_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("stats.db");
    let writer = StatsRecorder::new(&path).unwrap();
    writer
        .record(
            &StatsRecord::new(OperationType::RewriteCommand, "fixture".into(), 12, 6, 4, 2)
                .with_session_id("session & value")
                .with_text("before".into(), "after".into()),
        )
        .unwrap();
    drop(writer);
    let state = web::Data::new(State {
        database: path.clone(),
        credential: blake3::hash(b"fixture-token"),
    });
    let app = test::init_service(
        App::new()
            .app_data(state)
            .default_service(web::route().to(query)),
    )
    .await;
    for uri in [
        "/v1/stats/records",
        "/v1/stats/records/1?include_content=true",
    ] {
        for token in [None, Some("Bearer wrong"), Some("Basic fixture-token")] {
            let mut request = test::TestRequest::get().uri(uri);
            if let Some(token) = token {
                request = request.insert_header((header::AUTHORIZATION, token));
            }
            let response = test::call_service(&app, request.to_request()).await;
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            assert_eq!(
                response.headers().get(header::WWW_AUTHENTICATE).unwrap(),
                "Bearer"
            );
        }
    }
    let duplicate = test::TestRequest::get()
        .uri("/v1/stats/records")
        .append_header((header::AUTHORIZATION, "Bearer fixture-token"))
        .append_header((header::AUTHORIZATION, "Bearer fixture-token"))
        .to_request();
    assert_eq!(
        test::call_service(&app, duplicate).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let request = test::TestRequest::get()
        .uri("/v1/stats/records?session_id=session%20%26%20value")
        .insert_header((header::AUTHORIZATION, "bearer fixture-token"))
        .to_request();
    let value: Value = test::call_and_read_body_json(&app, request).await;
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["records"][0]["session_id"], "session & value");
    assert_eq!(value["records"][0]["before_text"], Value::Null);
    for (uri, expected) in [
        ("/v1/stats/records?limit=0", StatusCode::BAD_REQUEST),
        ("/v1/stats/records?unknown=1", StatusCode::BAD_REQUEST),
        ("/v1/stats/records/999", StatusCode::NOT_FOUND),
        (
            "/v1/stats/records/1?include_content=wrong",
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let request = test::TestRequest::get()
            .uri(uri)
            .insert_header((header::AUTHORIZATION, "Bearer fixture-token"))
            .to_request();
        assert_eq!(test::call_service(&app, request).await.status(), expected);
    }
    let request = test::TestRequest::get()
        .uri("/v1/stats/records/1?include_content=true")
        .insert_header((header::AUTHORIZATION, "Bearer fixture-token"))
        .to_request();
    let value: Value = test::call_and_read_body_json(&app, request).await;
    assert_eq!(value["record"]["before_text"], "before");
    assert_eq!(value["record"]["content_included"], true);
    let request = test::TestRequest::post()
        .uri("/v1/stats/records")
        .insert_header((header::AUTHORIZATION, "Bearer fixture-token"))
        .to_request();
    assert_eq!(
        test::call_service(&app, request).await.status(),
        StatusCode::METHOD_NOT_ALLOWED
    );
    std::fs::remove_file(path).unwrap();
    let request = test::TestRequest::get()
        .uri("/v1/stats/records")
        .insert_header((header::AUTHORIZATION, "Bearer fixture-token"))
        .to_request();
    assert_eq!(
        test::call_service(&app, request).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[actix_web::test]
async fn credential_files_and_response_limits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("credential");
    for bytes in [
        Vec::new(),
        b"has space".to_vec(),
        vec![b'x'; 4097],
        vec![0xff],
    ] {
        std::fs::write(&path, bytes).unwrap();
        assert!(matches!(
            load_credential(&path),
            Err(ServerError::Credential)
        ));
    }
    std::fs::write(&path, b"fixture-token\n").unwrap();
    assert_eq!(
        load_credential(&path).unwrap(),
        blake3::hash(b"fixture-token")
    );
    assert_eq!(
        response(json!({"text":"x".repeat(MAX_RESPONSE_BYTES)})).status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert!(load_credential(&dir.path().join("missing")).is_err());
}
