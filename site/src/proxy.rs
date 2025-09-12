use axum::{body::Body, extract::Request, http::StatusCode, response::Response};
use futures::StreamExt;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1";
const SHELV_TOKEN: &str = "shelv-token";
const ANTHROPIC_API_KEY: &str = "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";

pub async fn proxy_anthropic(req: Request) -> Result<Response<Body>, (StatusCode, String)> {
    // Extract and verify the authorization header
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "Missing or invalid authorization header".to_string(),
        ))?;

    if !auth_header.contains(SHELV_TOKEN) {
        return Err((
            StatusCode::UNAUTHORIZED,
            format!("Are you trying to get delicious claude api not from Shelv?"),
        ));
    }

    let client = reqwest::Client::new();

    let (parts, body) = req.into_parts();
    let body_bytes = axum::body::to_bytes(body, usize::MAX).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to read request body: {}", e),
        )
    })?;

    let req = Request::from_parts(parts, Body::empty());

    let mut anthropic_request = client
        .request(req.method().clone(), ANTHROPIC_API_URL)
        .header("authorization", format!("Bearer {}", ANTHROPIC_API_KEY));

    // Add other headers (except authorization which we're replacing)
    for (key, value) in req.headers() {
        if key.as_str().to_lowercase() != "authorization" && key.as_str().to_lowercase() != "host" {
            anthropic_request = anthropic_request.header(key, value);
        }
    }

    if !body_bytes.is_empty() {
        anthropic_request = anthropic_request.body(body_bytes.to_vec());
    }

    let anthropic_response = anthropic_request.send().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("Failed to send request to Anthropic API: {}", e),
        )
    })?;

    let status = anthropic_response.status();
    let headers = anthropic_response.headers().clone();

    // Convert the streaming response to axum Body
    let stream = anthropic_response
        .bytes_stream()
        .map(|result| result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));

    let mut response = Response::builder().status(status);

    for (key, value) in headers.iter() {
        if key.as_str().to_lowercase() != "content-length" {
            response = response.header(key, value);
        }
    }

    let response = response.body(Body::from_stream(stream)).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to build response: {}", e),
        )
    })?;

    Ok(response)
}
