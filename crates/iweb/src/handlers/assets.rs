use axum::response::Html;

pub async fn index() -> Html<&'static str> {
    Html(include_str!("../../assets/index.html"))
}

pub async fn style() -> impl axum::response::IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/css")],
        include_str!("../../assets/style.css"),
    )
}

pub async fn explore_css() -> impl axum::response::IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/css")],
        include_str!("../../assets/explore.css"),
    )
}

pub async fn explore_js() -> impl axum::response::IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        include_str!("../../assets/explore.js"),
    )
}
