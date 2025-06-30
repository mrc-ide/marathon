//! This module provides the basic scaffolding used to encode responses from the API.
//!
//! The types in this module and the schema of the responses are generic and could be used in other
//! API implementations.

use anyhow::bail;
use axum::body::Body;
use axum::http::{Response, StatusCode};
use axum::response::IntoResponse;
use reqwest::header::CONTENT_TYPE;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "status", rename = "success")]
pub struct ApiSuccess<T> {
    pub data: T,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "status", rename = "failure")]
pub struct ApiFailure {
    pub errors: Vec<ApiError>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ApiError {
    pub error: String,
    pub detail: Option<String>,
}

/// The return type for Axum endpoint handlers.
///
/// `ApiResponse` values can either be created explicitly using functions such as
/// `ApiResponse::ok(...)` or `ApiResponse::not_found()`, or they can be created by propagating an
/// arbitrary error using the try `?` operator. Propagated errors are always considered unexpected
/// and are mapped to a 500 status code.
///
/// This needs to be a `Result` in order for the Try operator to work. If and when the
/// `FromResidual` trait is stabilised this can become its own struct or enum type, which will make
/// defining methods on it more straightforward.
pub type ApiResponse<T> = Result<ApiSuccessResponse<T>, ApiFailureResponse>;

pub struct ApiSuccessResponse<T> {
    body: ApiSuccess<T>,
    status: StatusCode,
}

pub struct ApiFailureResponse {
    body: ApiFailure,
    status: StatusCode,
}

impl<T: Serialize> IntoResponse for ApiSuccessResponse<T> {
    fn into_response(self) -> Response<Body> {
        let mut response = axum::Json(&self.body).into_response();
        *response.status_mut() = self.status;
        response
    }
}

impl IntoResponse for ApiFailureResponse {
    fn into_response(self) -> Response<Body> {
        let mut response = axum::Json(&self.body).into_response();
        *response.status_mut() = self.status;
        response
    }
}

/// This trait provides extension methods on the `ApiResponse` type.
pub trait ApiResponseExt<T> {
    /// Return a 200 OK response
    ///
    /// Result already has an `ok` method which would clash, hence the alternative name.
    fn success(data: T) -> Self;

    /// Return a 204 Created response
    fn created(data: T) -> Self;

    /// Return a 404 Not Found error
    fn not_found() -> Self;
}
impl<T> ApiResponseExt<T> for ApiResponse<T> {
    fn success(data: T) -> Self {
        Ok(ApiSuccessResponse {
            body: ApiSuccess { data },
            status: StatusCode::OK,
        })
    }

    fn created(data: T) -> Self {
        Ok(ApiSuccessResponse {
            body: ApiSuccess { data },
            status: StatusCode::CREATED,
        })
    }

    fn not_found() -> Self {
        Err(ApiFailureResponse {
            body: ApiFailure {
                errors: vec![ApiError {
                    error: "NOT_FOUND".to_owned(),
                    detail: None,
                }],
            },
            status: StatusCode::NOT_FOUND,
        })
    }
}

/// This trait implementation allows any error to be converted to an ApiResponse.
///
/// This is always considered an unhandled server error. Expected error should be created via
/// methods on ApiResponse.
impl<E: std::fmt::Display> From<E> for ApiFailureResponse {
    fn from(error: E) -> ApiFailureResponse {
        ApiFailureResponse {
            body: ApiFailure {
                errors: vec![ApiError {
                    error: "INTERNAL_SERVER_ERROR".to_owned(),
                    detail: Some(error.to_string()),
                }],
            },
            status: StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// Extension trait for reqwest's response type.
pub trait ResponseExt {
    /// Decode a response from the API and return its body.
    ///
    /// The top-level wrapper object is unpeeled and only the useful contents are returned. HTTP
    /// errors are decoded and returned as an `Err`.
    fn api_response<T: DeserializeOwned>(self) -> crate::Result<T>;
}
impl ResponseExt for reqwest::blocking::Response {
    fn api_response<T: DeserializeOwned>(self) -> crate::Result<T> {
        let code = self.status();

        if code.is_client_error() || code.is_server_error() {
            let content_type = self.headers().get(CONTENT_TYPE);
            let url = self.url().clone();
            if content_type.is_some_and(|v| v == "application/json") {
                let body = self.json::<ApiFailure>()?;
                if let Some(detail) = body.errors.first().and_then(|e| e.detail.as_ref()) {
                    bail!("API error {} for url {}: {}", code, url, detail);
                } else {
                    bail!("API error {} for url {}", code, url);
                }
            } else {
                bail!("HTTP status error {} for url {}", code, url);
            }
        }

        let body = self.json::<ApiSuccess<T>>()?;
        Ok(body.data)
    }
}
