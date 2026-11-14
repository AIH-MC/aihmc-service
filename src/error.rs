use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

pub struct ApiError {
    pub status: StatusCode,
    pub detail: Value,
}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    pub fn message(status: u16, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::from_u16(status).unwrap(),
            detail: json!({"message": message.into()}),
        }
    }

    pub fn internal(error: impl std::fmt::Display) -> Self {
        tracing::error!(%error, "request failed");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            detail: Value::Null,
        }
    }

    pub fn validation(errors: Vec<Value>) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            detail: Value::Array(errors),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if self.status == StatusCode::INTERNAL_SERVER_ERROR {
            return (self.status, "Internal Server Error").into_response();
        }
        (self.status, Json(json!({"detail": self.detail}))).into_response()
    }
}
