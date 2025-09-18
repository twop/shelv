use axum::{
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    response::Response,
};
use futures::StreamExt;
use std::sync::Arc;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";

#[derive(Clone)]
pub struct Config {
    pub shelv_magic_token: String,
    pub anthropic_api_key: Option<String>,
}

pub async fn proxy_anthropic(
    config: &Config,
    req: Request,
) -> Result<Response<Body>, (StatusCode, String)> {
    println!("proxy_anthropic req: {req:#?}");
    // Short-circuit if no API key is configured
    let anthropic_api_key = match &config.anthropic_api_key {
        Some(key) if key.is_empty() => {
            println!("Err proxy llm req: Anthropic API key is empty");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Anthropic API key is empty".to_string(),
            ));
        }
        None => {
            println!("Err proxy llm req: Anthropic API key not configured");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Anthropic API key not configured".to_string(),
            ));
        }
        Some(key) => key,
    };

    // Extract and verify the authorization header
    let auth_header = req
        .headers()
        .get("x-api-key")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            println!("Err proxy llm req: Missing or invalid authorization header");
            (
                StatusCode::UNAUTHORIZED,
                "Missing or invalid authorization header".to_string(),
            )
        })?;

    if !auth_header.contains(&config.shelv_magic_token) {
        println!("Err proxy llm req: Invalid magic token in auth header");
        return Err((
            StatusCode::UNAUTHORIZED,
            format!("Are you trying to get delicious claude api not from Shelv?"),
        ));
    }

    let client = reqwest::Client::new();

    let (parts, body) = req.into_parts();
    let body_bytes = axum::body::to_bytes(body, usize::MAX).await.map_err(|e| {
        println!("Err proxy llm req: Failed to read request body: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to read request body: {}", e),
        )
    })?;

    let req = Request::from_parts(parts, Body::empty());

    let mut anthropic_request = client
        .request(req.method().clone(), ANTHROPIC_API_URL)
        .header("x-api-key", format!("{}", anthropic_api_key));

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
        println!("Err proxy llm req: Failed to send request to Anthropic API: {e:#?}");
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
        println!("Err proxy llm req: Failed to build response: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to build response: {}", e),
        )
    })?;

    println!("Ok: Successfully proxied request to Anthropic API");
    Ok(response)
}
