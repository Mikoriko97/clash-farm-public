//! Adds `request_id` to error bodies. Every response already carries
//! `X-Request-Id` (tower-http); this keeps the JSON envelope complete.

use axum::body::Body;
use axum::extract::Request;
use axum::http::header::CONTENT_TYPE;
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;

use crate::error::{ErrorBody, ErrorEnvelope, ErrorInner};

pub async fn finalize_errors(request: Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let response = next.run(request).await;
    let Some(envelope) = response.extensions().get::<ErrorEnvelope>().cloned() else {
        return response;
    };
    let body = ErrorBody {
        error: ErrorInner {
            code: envelope.code,
            message: &envelope.message,
            request_id: request_id.as_deref(),
        },
    };
    let Ok(json) = serde_json::to_vec(&body) else {
        return response;
    };
    let (mut parts, _) = response.into_parts();
    parts
        .headers
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    parts.headers.remove("content-length");
    Response::from_parts(parts, Body::from(json))
}
