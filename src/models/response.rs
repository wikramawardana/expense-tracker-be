use axum::{
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub status: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T> ApiResponse<T>
where
    T: Serialize,
{
    pub fn new(status: &str, message: &str, data: Option<T>) -> Self {
        Self {
            status: status.to_string(),
            message: message.to_string(),
            data,
        }
    }

    pub fn success(data: T, message: &str) -> Self {
        Self::new("success", message, Some(data))
    }

    pub fn success_msg(message: &str) -> Self {
        Self::new("success", message, None)
    }

    pub fn error(message: &str) -> Self {
        Self::new("error", message, None)
    }
}

impl<T> IntoResponse for ApiResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        Json(self).into_response()
    }
}
