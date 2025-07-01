use axum::body::Body;
use axum::http::Response;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum ApiResponse<T> {
    #[serde(rename = "success")]
    Success { data: T },
}
impl<T> ApiResponse<T> {
    pub fn success(data: T) -> ApiResponse<T> {
        ApiResponse::Success { data }
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response<Body> {
        axum::Json(self).into_response()
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct TaskSubmitResponse {
    pub id: Uuid,
}
