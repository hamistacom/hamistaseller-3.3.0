//! One error type that turns into a clean JSON HTTP response. Messages are Persian (shown to users).
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub message: String,
}

pub type AppResult<T> = Result<T, AppError>;

pub fn bad(msg: impl Into<String>) -> AppError {
    AppError { status: StatusCode::INTERNAL_SERVER_ERROR, message: msg.into() }
}
pub fn client(status: StatusCode, msg: impl Into<String>) -> AppError {
    AppError { status, message: msg.into() }
}
pub fn unauthorized(msg: impl Into<String>) -> AppError {
    client(StatusCode::UNAUTHORIZED, msg)
}
pub fn forbidden(msg: impl Into<String>) -> AppError {
    client(StatusCode::FORBIDDEN, msg)
}
pub fn not_found(msg: impl Into<String>) -> AppError {
    client(StatusCode::NOT_FOUND, msg)
}
pub fn invalid(msg: impl Into<String>) -> AppError {
    client(StatusCode::BAD_REQUEST, msg)
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if self.status.is_server_error() {
            tracing::error!("{}", self.message);
        }
        (self.status, Json(json!({ "ok": false, "error": self.message }))).into_response()
    }
}

// rusqlite / pool errors bubble up as 500 with their text.
impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        bad(format!("خطای پایگاه داده: {e}"))
    }
}
impl From<r2d2::Error> for AppError {
    fn from(e: r2d2::Error) -> Self {
        bad(format!("اتصال پایگاه داده: {e}"))
    }
}
