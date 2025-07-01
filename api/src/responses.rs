//! This module provides the basic scaffolding used to encode responses from the API.
//!
//! The types in this module and the schema of the responses are generic and could be used in other
//! API implementations.

use axum::http::StatusCode;
use axum::response::{IntoResponse, IntoResponseParts, Response};
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
/// The type allows any `P: IntoResponseParts` type to be attached in order to customize what
/// headers are returned to the client.
///
/// This needs to be a `Result` in order for the Try operator to work. If and when the
/// `FromResidual` trait is stabilised this can become its own struct or enum type, which will make
/// defining methods on it more straightforward.
pub type ApiResponse<T, P = ()> =
    Result<ResponseWrapper<ApiSuccess<T>, P>, ResponseWrapper<ApiFailure, P>>;

pub struct ResponseWrapper<B, P> {
    body: B,
    status: StatusCode,
    parts: P,
}

impl<B: Serialize, P: IntoResponseParts> IntoResponse for ResponseWrapper<B, P> {
    fn into_response(self) -> Response {
        (self.status, self.parts, axum::Json(self.body)).into_response()
    }
}

impl<B> ResponseWrapper<B, ()> {
    fn with<P: IntoResponseParts>(self, parts: P) -> ResponseWrapper<B, P> {
        let ResponseWrapper {
            body,
            status,
            parts: (),
        } = self;

        ResponseWrapper {
            body,
            status,
            parts,
        }
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

    /// Return a 400 Bad Request error
    fn bad_request() -> Self;

    /// Attach headers or extensions to the response.
    ///
    /// Any value which implements `IntoResponseParts` can be used. In particular, this includes
    /// arrays of header name and value pairs.
    fn with<P: IntoResponseParts>(self, parts: P) -> ApiResponse<T, P>;
}
impl<T> ApiResponseExt<T> for ApiResponse<T> {
    fn success(data: T) -> Self {
        Ok(ResponseWrapper {
            body: ApiSuccess { data },
            status: StatusCode::OK,
            parts: (),
        })
    }

    fn created(data: T) -> Self {
        Ok(ResponseWrapper {
            body: ApiSuccess { data },
            status: StatusCode::CREATED,
            parts: (),
        })
    }

    fn not_found() -> Self {
        Err(ResponseWrapper {
            body: ApiFailure {
                errors: vec![ApiError {
                    error: "NOT_FOUND".to_owned(),
                    detail: None,
                }],
            },
            status: StatusCode::NOT_FOUND,
            parts: (),
        })
    }

    fn bad_request() -> Self {
        Err(ResponseWrapper {
            body: ApiFailure {
                errors: vec![ApiError {
                    error: "BAD_REQUEST".to_owned(),
                    detail: None,
                }],
            },
            status: StatusCode::BAD_REQUEST,
            parts: (),
        })
    }

    fn with<P: IntoResponseParts>(self, parts: P) -> ApiResponse<T, P> {
        match self {
            Ok(inner) => Ok(inner.with(parts)),
            Err(inner) => Err(inner.with(parts)),
        }
    }
}

/// This trait implementation allows any error to be converted to an ApiResponse.
///
/// This is always considered an unhandled server error. Expected error should be created via
/// methods on ApiResponse.
impl<E: std::fmt::Display, P> From<E> for ResponseWrapper<ApiFailure, P>
where
    P: Default + IntoResponseParts,
{
    fn from(error: E) -> ResponseWrapper<ApiFailure, P> {
        ResponseWrapper {
            body: ApiFailure {
                errors: vec![ApiError {
                    error: "INTERNAL_SERVER_ERROR".to_owned(),
                    detail: Some(error.to_string()),
                }],
            },
            status: StatusCode::INTERNAL_SERVER_ERROR,
            parts: P::default(),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ApiClientError {
    HTTPError {
        status: StatusCode,
        url: reqwest::Url,
        body: Option<ApiFailure>,
    },
    Other(#[from] reqwest::Error),
}

impl ApiClientError {
    pub fn has_status(&self, expected: StatusCode) -> bool {
        match self {
            ApiClientError::HTTPError { status, .. } => *status == expected,
            ApiClientError::Other(inner) => {
                // The reqwest::Error in the Other variant is used for for I/O and deserialization
                // errors, not for error HTTP status codes.
                assert!(inner.status().is_none());
                false
            }
        }
    }
}

impl std::fmt::Display for ApiClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiClientError::Other(e) => e.fmt(f),
            ApiClientError::HTTPError { status, url, body } => {
                if let Some(detail) = body
                    .as_ref()
                    .and_then(|b| b.errors.first())
                    .and_then(|e| e.detail.as_ref())
                {
                    write!(f, "API error {status} for url {url}: {detail}")
                } else {
                    write!(f, "API error {status} for url {url}")
                }
            }
        }
    }
}

/// Extension trait for reqwest's response type.
pub trait ResponseExt {
    /// Decode a response from the API and return its body.
    ///
    /// The top-level wrapper object is unpeeled and only the useful contents are returned. HTTP
    /// errors are decoded and returned as an `Err`.
    fn api_response<T: DeserializeOwned>(self) -> Result<T, ApiClientError>;
}
impl ResponseExt for reqwest::blocking::Response {
    fn api_response<T: DeserializeOwned>(self) -> Result<T, ApiClientError> {
        let status = self.status();

        if status.is_client_error() || status.is_server_error() {
            let content_type = self.headers().get(CONTENT_TYPE);
            let url = self.url().clone();
            if content_type.is_some_and(|v| v == "application/json") {
                // We ignore errors decoding the body. If there are any we just use None.
                let body = self.json::<ApiFailure>().ok();
                return Err(ApiClientError::HTTPError { status, url, body });
            } else {
                return Err(ApiClientError::HTTPError {
                    status,
                    url,
                    body: None,
                });
            }
        }

        let body = self.json::<ApiSuccess<T>>()?;
        Ok(body.data)
    }
}
