use axum::Router;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable};
use utoipa_swagger_ui::SwaggerUi;

use super::route;
use crate::{
    app::{AppState, middleware},
    error::Result,
};

/// Create the application router with all routes and middleware
pub async fn create_router(state: AppState) -> Result<Router> {
    let router = OpenApiRouter::new()
        .merge(route::auth::router())
        .merge(route::chore::router())
        .nest("/users", route::user::router());

    let (router, api) = OpenApiRouter::new().nest("/api", router).split_for_parts();

    Ok(router
        .merge(Scalar::with_url("/api-docs/scalar", api.clone()))
        .merge(SwaggerUi::new("/api-docs/swagger-ui").url("/api-docs/openapi.json", api.clone()))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(middleware::auth::session(&state).await?)
                .layer(middleware::cors::cors()),
        )
        .with_state(state))
}
