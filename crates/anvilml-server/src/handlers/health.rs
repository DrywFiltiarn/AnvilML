/// Liveness-check handler.
///
/// Returns `200 OK` with an empty body. Used by orchestrators and
/// load balancers to verify that the server process is alive and
/// able to accept connections.
pub async fn health() -> axum::http::StatusCode {
    axum::http::StatusCode::OK
}
