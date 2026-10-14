//! `/api/files`：上传、挂载与列表；媒体下发在 `app/routes/media.rs`。

use axum::{Json, extract::State, response::IntoResponse};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    app::{
        AppContext,
        dto::{FileIdPath, FileListResp, FileResp, PaginationReq, UploadForm},
        extractor::{AppPath, AppQuery, FormData, Require},
        http::{error::ErrorResponses, openapi::OpenApiRouterExt},
        identity::{Auth, JwtOrSession},
        middleware::auth::auth,
    },
    domain::vo::{FileId, Pagination, UserWrite},
    error::AppError,
};

#[utoipa::path(get, path="/", params(PaginationReq), responses(
    ErrorResponses,
    (status = 200, body = FileListResp),
))]
pub async fn list(
    _perm: Require<UserWrite>,
    Auth(auth): Auth<JwtOrSession>,
    State(ctx): State<AppContext>,
    AppQuery(pagination): AppQuery<PaginationReq>,
) -> Result<impl IntoResponse, AppError> {
    let pagination = Pagination::try_from(pagination)?;

    let files = ctx
        .srv()
        .file
        .page(auth.user_id, pagination)
        .await?
        .map(|stored| FileResp::from_stored(&stored, &ctx.cfg().storage.media_url_prefix));

    Ok(Json(FileListResp::from(files)))
}

#[utoipa::path(post, path="/", request_body(content = UploadForm, content_type = "multipart/form-data"), responses(
    ErrorResponses,
    (status = 200, body = FileResp),
))]
pub async fn upload(
    _perm: Require<UserWrite>,
    Auth(auth): Auth<JwtOrSession>,
    State(ctx): State<AppContext>,
    FormData(form): FormData<UploadForm>,
) -> Result<impl IntoResponse, AppError> {
    let stored = ctx
        .srv()
        .file
        .upload(
            auth.user_id,
            form.file.name,
            form.file.mime_type,
            &form.file.data,
        )
        .await?;

    Ok(Json(FileResp::from_stored(
        &stored,
        &ctx.cfg().storage.media_url_prefix,
    )))
}

#[utoipa::path(post, path="/{id}/attach", params(
    ("id" = FileId, Path, description = "Opaque file handle"),
), responses(
    ErrorResponses,
    (status = 200, body = FileResp),
))]
pub async fn attach(
    _perm: Require<UserWrite>,
    Auth(auth): Auth<JwtOrSession>,
    State(ctx): State<AppContext>,
    AppPath(FileIdPath { id }): AppPath<FileIdPath>,
) -> Result<impl IntoResponse, AppError> {
    let stored = ctx.srv().file.attach(auth.user_id, id).await?;

    Ok(Json(FileResp::from_stored(
        &stored,
        &ctx.cfg().storage.media_url_prefix,
    )))
}

pub const PATH: &str = "/files";

pub fn router(ctx: &AppContext) -> OpenApiRouter<AppContext> {
    OpenApiRouter::new()
        .routes(routes![list])
        .routes(routes![upload])
        .routes(routes![attach])
        .with_tags(["files"])
        .route_layer(auth(ctx, JwtOrSession))
}
