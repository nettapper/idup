use askama::Template;
use axum::body::Body;
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};

#[derive(Template)]
#[template(path = "fragments/error.html")]
struct ErrorFragment {
    message: String,
}

#[derive(Template)]
#[template(path = "error_page.html")]
struct ErrorPage {
    message: String,
}

#[allow(dead_code)]
pub enum AppError {
    /// Database error
    Db(sqlx::Error),
    /// Resource not found
    NotFound(String),
    /// Access denied
    Forbidden(String),
    /// Internal error
    Internal(String),
}

impl AppError {
    /// Render as an HTML fragment (for htmx responses)
    pub fn into_fragment(self) -> Html<String> {
        let message = self.message();
        let tmpl = ErrorFragment { message };
        Html(
            tmpl.render()
                .unwrap_or_else(|_| r#"<div class="result-error">Error</div>"#.to_string()),
        )
    }

    /// Render as a full error page (for standalone pages like /explore)
    pub fn into_page(self) -> Html<String> {
        let message = self.message();
        let tmpl = ErrorPage {
            message: message.clone(),
        };
        Html(tmpl.render().unwrap_or_else(|_| format!("<p>Error: {}</p>", message)))
    }

    pub fn message(&self) -> String {
        match self {
            AppError::Db(e) => e.to_string(),
            AppError::NotFound(msg) => msg.clone(),
            AppError::Forbidden(msg) => msg.clone(),
            AppError::Internal(msg) => msg.clone(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::Db(_) | AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
        };
        let message = self.message();
        Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::from(message))
            .unwrap()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::Db(e)
    }
}

impl From<askama::Error> for AppError {
    fn from(e: askama::Error) -> Self {
        AppError::Internal(format!("Template error: {}", e))
    }
}
