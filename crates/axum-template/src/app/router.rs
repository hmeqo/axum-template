use axum::{Router, extract::DefaultBodyLimit};
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable};
use utoipa_swagger_ui::SwaggerUi;

use super::{handler, middleware::cors};
use crate::{app::AppState, config::RawAppConfig, error::Result, ext::EndpointRouterT};

/// Create the application router with all routes and middleware
pub async fn create_router(state: AppState) -> Result<Router> {
    let router = OpenApiRouter::new()
        .merge(handler::chore::router())
        .mount(handler::auth::router())
        .mount(handler::user::router())
        .mount(handler::jwt_demo::router());

    let (router, api) = OpenApiRouter::new().nest("/api", router).split_for_parts();

    Ok(router
        .merge(Scalar::with_url("/api-docs/scalar", api.clone()))
        .merge(SwaggerUi::new("/api-docs/swagger-ui").url("/api-docs/openapi.json", api.clone()))
        .layer(
            ServiceBuilder::new()
                .layer(DefaultBodyLimit::max(
                    state.cfg().storage.max_file_size as usize,
                ))
                .layer(TraceLayer::new_for_http())
                .layer(cors::cors()),
        )
        .merge(handler::html::router())
        .with_state(state))
}

/// Create TCP listener for the server
pub async fn create_listener(config: &RawAppConfig) -> Result<TcpListener> {
    let addr = format!("{}:{}", config.server.host, config.server.port);
    Ok(TcpListener::bind(&addr).await?)
}
