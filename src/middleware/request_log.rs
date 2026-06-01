use std::time::Instant;

use axum::{extract::Request, middleware::Next, response::Response};
use tracing::{error, info, warn};

pub async fn request_log(request: Request, next: Next) -> Response {
    let started_at = Instant::now();
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let query = request.uri().query().map(str::to_string);

    let response = next.run(request).await;
    let status = response.status();
    let latency_ms = started_at.elapsed().as_millis() as u64;

    if should_log(&path) {
        let path_with_query = match query {
            Some(query) => format!("{path}?{query}"),
            None => path,
        };

        if status.is_server_error() {
            error!(
                event = "backend_request",
                method = %method,
                path = %path_with_query,
                status = status.as_u16(),
                latency_ms,
                "request completed"
            );
        } else if status.is_client_error() {
            warn!(
                event = "backend_request",
                method = %method,
                path = %path_with_query,
                status = status.as_u16(),
                latency_ms,
                "request completed"
            );
        } else {
            info!(
                event = "backend_request",
                method = %method,
                path = %path_with_query,
                status = status.as_u16(),
                latency_ms,
                "request completed"
            );
        }
    }

    response
}

fn should_log(path: &str) -> bool {
    !matches!(path, "/api/v1/health" | "/metrics")
}
