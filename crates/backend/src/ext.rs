use utoipa::openapi::OpenApi;
use utoipa_axum::router::OpenApiRouter;

use crate::app::AppState;

pub trait OpenApiRouterExt {
    fn with_tags<T: Into<String>>(self, tags: impl IntoIterator<Item = T>) -> Self;
}

impl OpenApiRouterExt for OpenApiRouter<AppState> {
    fn with_tags<T: Into<String>>(mut self, tags: impl IntoIterator<Item = T>) -> Self {
        set_tags(
            self.get_openapi_mut(),
            tags.into_iter().map(|t| t.into()).collect(),
        );
        self
    }
}

pub fn set_tags(openapi: &mut OpenApi, tags: Vec<String>) {
    for (_, path_item) in openapi.paths.paths.iter_mut() {
        for method in [
            &mut path_item.get,
            &mut path_item.post,
            &mut path_item.put,
            &mut path_item.delete,
        ]
        .into_iter()
        .flatten()
        {
            method
                .tags
                .get_or_insert_with(Vec::new)
                .extend(tags.clone());
        }
    }
}

/// (Endpoint, OpenApiRouter)
pub struct EndpointRouter<S>(pub &'static str, pub OpenApiRouter<S>);

pub trait EndpointRouterT<S> {
    fn mount(self, pr: EndpointRouter<S>) -> Self;

    fn endpoint(self, endpoint: &'static str) -> EndpointRouter<S>;
}

impl<S: Send + Sync + Clone + 'static> EndpointRouterT<S> for OpenApiRouter<S> {
    fn mount(self, pr: EndpointRouter<S>) -> Self {
        self.nest(pr.0, pr.1)
    }

    fn endpoint(self, endpoint: &'static str) -> EndpointRouter<S> {
        EndpointRouter(endpoint, self)
    }
}
