use clap::Parser;

mod handlers;

#[derive(Debug, Parser)]
#[command(
    name = "iweb",
    about = "Web UI for idup — browse and manage duplicate images"
)]
struct Cli {
    /// Port to listen on
    #[arg(short, long, default_value_t = 3000)]
    port: u16,
    /// Open the browser automatically after starting
    #[arg(long)]
    open: bool,
}

#[tokio::main]
async fn main() -> sqlx::Result<()> {
    let cli = Cli::parse();
    let pool = idup::db::open_pool().await?;
    serve(cli.port, cli.open, pool).await;
    Ok(())
}

async fn serve(port: u16, open_browser: bool, pool: sqlx::SqlitePool) {
    use axum::routing::{get, post};
    use axum::Router;
    use tower_http::cors::CorsLayer;

    let app = Router::new()
        .route("/", get(handlers::index))
        .route("/style.css", get(handlers::style))
        .route("/api/scan", post(handlers::scan))
        .route("/api/extract", post(handlers::extract))
        .route("/api/update", post(handlers::update))
        .route("/api/clean", post(handlers::clean))
        .route("/api/crop", post(handlers::crop))
        .route("/api/list", get(handlers::list))
        .route("/api/info", get(handlers::info))
        .route("/api/random", get(handlers::random))
        .route("/api/image", get(handlers::image_file))
        .route("/explore", get(handlers::explore))
        .with_state(pool)
        .layer(CorsLayer::permissive());

    let addr = format!("0.0.0.0:{port}");
    let url = format!("http://localhost:{port}");

    println!("iweb → {url}");

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
