# Tokenless Statistics Recording

[中文版](tokenless-statistics_zh.md)

The server queries a configured Tokenless API with a backend-only credential. It saves
bounded, atomic results in the existing GenAI store to reuse lifecycle and retention policies.
Original source fields remain unchanged; local session association requires a unique captured tool-call ID. Missing or ambiguous matches stay null. Endpoint, source ID and original timestamp
deduplicate refreshes without introducing a recovery protocol. Metadata refresh preserves cached
content. Failures are explicit; saved statistics remain readable when the source is offline.

The display layer uses cached records for one configured endpoint. Applied savings require
an explicit active mode and a unique captured association. Content fetching verifies the source
ID and original timestamp before updating a copy. The larger auth/savings handler files receive
only delegated entry points; their decomposition plan is to move auth handlers, savings DTOs and
tests into sibling modules while preserving routes. New transport and view logic stays in the
Tokenless modules. The Linux `make test-tokenless-api` gate exercises both built CLI processes;
build Tokenless and AgentSight first, and run it with Python 3.11 or newer.
