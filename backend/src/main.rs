use tokio::net::TcpListener;

use anvilml_server::{build_router, AppState};

#[tokio::main]
async fn main() {
    let state = AppState::new(env!("CARGO_PKG_VERSION"));
    let router = build_router(state);
    let listener = TcpListener::bind("127.0.0.1:8488")
        .await
        .expect("Failed to bind port 8488");
    println!("Listening on http://127.0.0.1:8488");
    let _ = axum::serve(listener, router).await;
}
