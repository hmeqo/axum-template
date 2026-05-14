use axum::{
    Router,
    body::Body,
    extract::Path,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use rust_embed::RustEmbed;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable};
use utoipa_swagger_ui::SwaggerUi;

use super::controller;
use crate::{
    app::AppState,
    config::AppConfig,
    error::Result,
    ext::EndpointRouterT,
};

use super::middleware::cors;

#[derive(RustEmbed)]
#[folder = "static/"]
struct Assets;

async fn static_handler(Path(path): Path<String>) -> impl IntoResponse {
    let path = path.trim_start_matches('/');

    if path.is_empty() {
        return index_handler().await.into_response();
    }

    // /a/index.html → redirect to /a/  (avoid duplicate content)
    if let Some(prefix) = path.strip_suffix("index.html") {
        let redirect_path = if prefix.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", prefix.trim_end_matches('/'))
        };
        return Response::builder()
            .status(StatusCode::FOUND)
            .header(header::LOCATION, redirect_path)
            .body(Body::empty())
            .unwrap();
    }

    // 1. Exact file match
    if let Some(content) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return Response::builder()
            .header(header::CONTENT_TYPE, mime.as_ref())
            .body(Body::from(content.data))
            .unwrap();
    }

    // 2. Path without extension → try as directory with index.html
    //    e.g. /docs → /docs/index.html
    if std::path::Path::new(path).extension().is_none() && !path.ends_with('/') {
        let index_path = format!("{}/index.html", path);
        if let Some(content) = Assets::get(&index_path) {
            return Response::builder()
                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                .body(Body::from(content.data))
                .unwrap();
        }
    }

    // 3. SPA fallback: every unmatched route serves index.html
    index_handler().await.into_response()
}

async fn index_handler() -> impl IntoResponse {
    match Assets::get("index.html") {
        Some(content) => Response::builder()
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(content.data))
            .unwrap(),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("404 Not Found"))
            .unwrap(),
    }
}

/// Create the application router with all routes and middleware
pub async fn create_router(state: AppState) -> Result<Router> {
    let router = OpenApiRouter::new()
        .merge(controller::chore::router())
        .mount(controller::auth::router())
        .mount(controller::user::router());

    let (router, api) = OpenApiRouter::new().nest("/api", router).split_for_parts();

    Ok(router
        .merge(Scalar::with_url("/api-docs/scalar", api.clone()))
        .merge(SwaggerUi::new("/api-docs/swagger-ui").url("/api-docs/openapi.json", api.clone()))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(cors::cors()),
        )
        .route("/", get(index_handler))
        .route("/{*path}", get(static_handler))
        .with_state(state))
}

/// Create TCP listener for the server
pub async fn create_listener(config: &AppConfig) -> Result<TcpListener> {
    let addr = format!("{}:{}", config.server.host, config.server.port);
    Ok(TcpListener::bind(&addr).await?)
}
