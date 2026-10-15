//! 路由与文档装配：路径、标签与全局中间件顺序在此处决定，组认证层由各路由模块自挂。

use axum::{Router, extract::DefaultBodyLimit, http::StatusCode};
use tower::ServiceBuilder;
use tower_http::{
    timeout::TimeoutLayer,
    trace::{DefaultOnResponse, TraceLayer},
};
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable};
use utoipa_swagger_ui::SwaggerUi;

use super::{
    AppContext,
    extractor::client_info,
    http::openapi::{
        describe_service, prefix_operation_ids, register_error_schemas, register_security_schemes,
    },
    middleware::{cors, error_log, no_store, request_id},
    routes, rpc,
    socket::realtime,
};
use crate::error::Result;

pub async fn create_router(ctx: AppContext) -> Result<Router> {
    let trusted = ctx.trusted_proxies().clone();
    let api = OpenApiRouter::new()
        .merge(rpc::router(&ctx))
        .nest(routes::auth::PATH, routes::auth::router(&ctx))
        .nest(routes::jwt::PATH, routes::jwt::router(&ctx))
        .nest(routes::user::PATH, routes::user::router(&ctx))
        .nest(routes::files::PATH, routes::files::router(&ctx))
        .nest(routes::error::PATH, routes::error::router())
        .fallback(routes::api_not_found)
        .layer(axum::middleware::from_fn(no_store::attach))
        .layer(axum::middleware::from_fn(request_id::attach));

    let (router, mut api_doc) = OpenApiRouter::new().nest("/api", api).split_for_parts();
    prefix_operation_ids(&mut api_doc);
    describe_service(&mut api_doc);
    register_error_schemas(&mut api_doc);
    register_security_schemes(&mut api_doc, &ctx.cfg().auth.session.cookie_name);

    Ok(router
        .merge(routes::health::router())
        .merge(routes::media::router(&ctx).layer(axum::middleware::from_fn(request_id::attach)))
        .merge(realtime::router(&ctx))
        .merge(Scalar::with_url("/api-docs/scalar", api_doc.clone()))
        .merge(
            SwaggerUi::new("/api-docs/swagger-ui").url("/api-docs/openapi.json", api_doc.clone()),
        )
        .merge(rpc::schema_router())
        .merge(routes::static_site::router())
        .fallback(routes::static_site::spa)
        .layer(
            ServiceBuilder::new()
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(move |req: &axum::http::Request<_>| {
                            let ip = client_info::ip_from_request(req, &trusted);
                            tracing::info_span!(
                                "request",
                                method = %req.method(),
                                uri = %req.uri(),
                                version = ?req.version(),
                                client_ip = ?ip,
                            )
                        })
                        // 访问日志一行/请求:INFO 级,生产(`log.level=info`)下不会消失。
                        .on_response(DefaultOnResponse::new().level(tracing::Level::INFO)),
                )
                .layer(cors::cors(&ctx.cfg().cors))
                .layer(TimeoutLayer::with_status_code(
                    StatusCode::REQUEST_TIMEOUT,
                    std::time::Duration::from_secs(ctx.cfg().server.request_timeout_seconds),
                ))
                .layer(DefaultBodyLimit::max(
                    ctx.cfg().storage.request_body_limit(),
                ))
                .layer(axum::middleware::from_fn(error_log::log_app_errors)),
        )
        .with_state(ctx))
}
