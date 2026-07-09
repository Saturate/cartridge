use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

pub async fn auth_middleware(
    request: Request,
    next: Next,
) -> Response {
    let token = request
        .extensions()
        .get::<Option<String>>()
        .cloned()
        .flatten();

    let Some(expected) = token else {
        return next.run(request).await;
    };

    let path = request.uri().path();
    if path == "/api/health" || path == "/api/openapi.json" || path == "/api/docs" || path == "/api/logs" {
        return next.run(request).await;
    }

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let query_token = request
        .uri()
        .query()
        .and_then(|q| {
            q.split('&')
                .find_map(|pair| pair.strip_prefix("token="))
        });

    let provided = auth_header.or(query_token);

    if provided == Some(&expected) {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": {
                    "code": "unauthorized",
                    "message": "Missing or invalid bearer token",
                    "status": 401
                }
            })),
        )
            .into_response()
    }
}
