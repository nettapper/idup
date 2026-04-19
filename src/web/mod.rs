mod handlers;

use axum::{
    routing::{get, post},
    Router,
};
use sqlx::SqlitePool;
use tower_http::cors::CorsLayer;

pub async fn serve(port: u16, open_browser: bool, pool: SqlitePool) {
    let app = Router::new()
        .route("/", get(handlers::index))
        .route("/style.css", get(handlers::style))
        .route("/api/scan", post(handlers::scan))
        .route("/api/list", get(handlers::list))
        .route("/api/info", get(handlers::info))
        .route("/api/compare", post(handlers::compare))
        .route("/api/random", get(handlers::random))
        .with_state(pool)
        .layer(CorsLayer::permissive());

    let addr = format!("0.0.0.0:{port}");
    let url = format!("http://localhost:{port}");

    println!("idup web → {url}");

    if open_browser {
        if let Err(e) = open::that(&url) {
            eprintln!("Could not open browser: {e}");
        }
    }

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {addr}: {e}"));

    axum::serve(listener, app)
        .await
        .unwrap_or_else(|e| panic!("Server error: {e}"));
}
