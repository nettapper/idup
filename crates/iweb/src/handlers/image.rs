use axum::extract::{Query, State};
use serde::Deserialize;
use sqlx::SqlitePool;

#[derive(Deserialize)]
pub struct ImageQuery {
    path: String,
}

pub async fn image_file(
    State(pool): State<SqlitePool>,
    Query(params): Query<ImageQuery>,
) -> axum::response::Response {
    use axum::body::Body;
    use axum::http::{header, StatusCode};

    // Gate: only serve paths tracked in the database OR paths starting with /tmp/nettapper/vid2img/
    let allowed = params.path.starts_with("/tmp/nettapper/vid2img/")
        || match idup::db::path_exists_in_db(&pool, &params.path).await {
            Ok(true) => true,
            Ok(false) => false,
            Err(e) => {
                return axum::response::Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body(Body::from(format!("DB error: {e}")))
                    .unwrap();
            }
        };

    if !allowed {
        return axum::response::Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::from("Path not in database and not a vid2img temp file"))
            .unwrap();
    }

    // Read the file from disk.
    let bytes = match tokio::fs::read(&params.path).await {
        Ok(b) => b,
        Err(_) => {
            return axum::response::Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(header::CONTENT_TYPE, "text/plain")
                .body(Body::from("File not found"))
                .unwrap();
        }
    };

    // Detect MIME type via magic bytes.
    let mime = infer::get(&bytes)
        .map(|t| t.mime_type())
        .unwrap_or("application/octet-stream");

    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "no-store, no-cache, must-revalidate")
        .body(Body::from(bytes))
        .unwrap()
}
