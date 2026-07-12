use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use crate::handlers;
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(handlers::assets::index))
        .route("/style.css", get(handlers::assets::style))
        .route("/explore.css", get(handlers::assets::explore_css))
        .route("/explore.js", get(handlers::assets::explore_js))
        .route("/api/scan", post(handlers::scan::scan))
        .route("/api/extract", post(handlers::extract::extract))
        .route("/api/update", post(handlers::update::update))
        .route("/api/clean", post(handlers::clean::clean))
        .route("/api/crop", post(handlers::crop::crop))
        .route("/api/explore/delete", post(handlers::file_ops::explore_delete))
        .route("/api/explore/move", post(handlers::file_ops::explore_move))
        .route("/api/list", get(handlers::list::list))
        .route("/api/info", get(handlers::info::info))
        .route("/api/random", get(handlers::random::random))
        .route("/api/image", get(handlers::image::image_file))
        .route("/explore", get(handlers::explore::explore))
        .with_state(state)
        .layer(CorsLayer::permissive())
}
