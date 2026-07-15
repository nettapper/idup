use axum::routing::{get, post};
use axum::Router;
use axum::extract::DefaultBodyLimit;
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
        .route("/api/vid2img/list", get(handlers::vid2img::list))
        .route("/api/vid2img/check_dir", get(handlers::vid2img::check_dir))
        .route("/api/vid2img/upload", post(handlers::vid2img::upload))
        .route("/api/vid2img/reextract", post(handlers::vid2img::reextract))
        .route("/api/vid2img/save", post(handlers::vid2img::save))
        .route("/api/vid2img/delete", post(handlers::vid2img::delete))
        .route("/explore", get(handlers::explore::explore))
        .with_state(state)
        .layer(DefaultBodyLimit::disable())
        .layer(CorsLayer::permissive())
}
