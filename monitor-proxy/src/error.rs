use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use log::error;
use serde_json::json;
use std::fmt::Display;

pub type Result<T, E = AppError> = std::result::Result<T, E>;

#[derive(Debug)]
pub struct AppError {
    status: StatusCode,
    message: String,
    inner: Option<anyhow::Error>,
}

pub trait AppErrorExt<T>
where
    Self: Sized,
{
    fn with_status<M>(self, status: StatusCode, message: &M) -> Result<T>
    where
        M: Display + ?Sized;
    fn internal_server_error<M>(self, message: &M) -> Result<T>
    where
        M: Display + ?Sized,
    {
        self.with_status(StatusCode::INTERNAL_SERVER_ERROR, message)
    }
    fn unauthorized<M>(self, message: &M) -> Result<T>
    where
        M: Display + ?Sized,
    {
        self.with_status(StatusCode::UNAUTHORIZED, message)
    }
    fn bad_request<M>(self, message: &M) -> Result<T>
    where
        M: Display + ?Sized,
    {
        self.with_status(StatusCode::BAD_REQUEST, message)
    }
    fn not_found<M>(self, message: &M) -> Result<T>
    where
        M: Display + ?Sized,
    {
        self.with_status(StatusCode::NOT_FOUND, message)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let error_msg = if let Some(e) = self.inner {
            format!("{}: {:#?}", self.message, e)
        } else {
            format!("{}", self.message)
        };
        error!(
            "Failed to process response ({} {}): {}",
            self.status.as_u16(),
            self.status.canonical_reason().unwrap_or("unknown"),
            error_msg
        );
        let body = Json(json!({
            "error": error_msg,
            "code": self.status.as_u16(),
        }));
        (self.status, body).into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("internal server error"),
            inner: Some(err.into()),
        }
    }
}

impl<T, E> AppErrorExt<T> for std::result::Result<T, E>
where
    E: Into<anyhow::Error>,
{
    fn with_status<M>(self, status: StatusCode, message: &M) -> Result<T>
    where
        M: Display + ?Sized,
    {
        self.map_err(|e| AppError {
            status: status,
            message: message.to_string(),
            inner: Some(e.into()),
        })
    }
}

impl<T> AppErrorExt<T> for std::option::Option<T> {
    fn with_status<M>(self, status: StatusCode, message: &M) -> Result<T>
    where
        M: Display + ?Sized,
    {
        self.ok_or_else(|| AppError {
            status: status,
            message: message.to_string(),
            inner: None,
        })
    }
}

impl AppError {
    pub fn new(status: StatusCode, message: impl Display) -> Self {
        Self {
            status,
            message: message.to_string(),
            inner: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    #[test]
    fn test_error_conversion() {
        let _err = AppError::from(anyhow!("test error"));
    }
}
