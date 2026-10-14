//! ConnectRPC 服务实现：每个方法取身份、调一次服务；需要权限的方法另查一次权限。

use std::future::Future;

use buffa::MessageField;
use buffa_types::google::protobuf::Empty;
use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest, ServiceResult,
};
use validator::Validate;

use super::{
    error::to_connect,
    proto::template::v1::{AuthState, ListUsersRequest, ListUsersResponse, TemplateService, User},
};
use crate::{
    app::{
        AppContext,
        dto::PaginationReq,
        identity::{AUTHENTICATION_REQUIRED, CurrentUser, Resolved},
    },
    domain::{
        model::User as UserRow,
        service::AuthUser,
        vo::{Pagination, Perm, UserFilter},
    },
};

pub struct RpcService {
    ctx: AppContext,
}

impl RpcService {
    pub(crate) fn new(ctx: AppContext) -> Self {
        Self { ctx }
    }

    /// 认证层写进请求扩展的身份；缺失或无效一律 401。
    fn identity(ctx: &RequestContext) -> Result<CurrentUser, ConnectError> {
        match Resolved::cached(ctx.extensions()) {
            Some(resolved) => {
                CurrentUser::of(&resolved).map_err(|e| to_connect(ctx.extensions(), e))
            }
            None => Err(ConnectError::unauthenticated(AUTHENTICATION_REQUIRED)),
        }
    }

    /// `Require<P>` 在 RPC 面的对应物。
    async fn authorize(
        &self,
        ctx: &RequestContext,
        user: CurrentUser,
        perm: Perm,
    ) -> Result<(), ConnectError> {
        self.ctx
            .srv()
            .auth
            .require_permission(user.user_id, perm)
            .await
            .map_err(|e| to_connect(ctx.extensions(), e))
    }
}

impl TemplateService for RpcService {
    /// 身份：`GET /api/auth/me` 的 RPC 版本。
    // 手写返回类型：`async fn` 会把请求体的生命周期一并捕获，超出 trait 的 `use<'a, Self>`。
    #[allow(clippy::manual_async_fn)]
    fn me<'a>(
        &'a self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, Empty>,
    ) -> impl Future<Output = ServiceResult<impl Encodable<AuthState> + Send + use<'a>>> + Send
    {
        async move {
            let user = Self::identity(&ctx)?;
            let auth_user = self
                .ctx
                .srv()
                .auth
                .get_auth_user(user.user_id)
                .await
                .map_err(|e| to_connect(ctx.extensions(), e))?;

            Response::ok(auth_state(auth_user))
        }
    }

    /// 用户列表：与 `GET /api/users` 同一套分页边界与过滤规则；proto3 的缺省 `0` 不是 REST 的“未给”，按越界拒绝。
    // 与 `Me` 同理：`async fn` 会连请求体的生命周期一起捕获。
    #[allow(clippy::manual_async_fn)]
    fn list_users<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ListUsersRequest>,
    ) -> impl Future<Output = ServiceResult<impl Encodable<ListUsersResponse> + Send + use<'a>>> + Send
    {
        async move {
            let user = Self::identity(&ctx)?;
            self.authorize(&ctx, user, Perm::UserRead).await?;

            let params = request.to_owned_message();
            let req = PaginationReq {
                page: params.page,
                per_page: params.per_page,
            };
            // 与 REST 的提取器同一步骤：先按 DTO 声明的边界校验，再交给值对象。
            req.validate()
                .map_err(|e| to_connect(ctx.extensions(), e.into()))?;
            let pagination =
                Pagination::try_from(req).map_err(|e| to_connect(ctx.extensions(), e))?;
            let filter = UserFilter::new(Some(params.username))
                .map_err(|e| to_connect(ctx.extensions(), e))?;

            let page = self
                .ctx
                .srv()
                .user
                .page(pagination, &filter)
                .await
                .map_err(|e| to_connect(ctx.extensions(), e))?;

            let (total, page_number, per_page) = (page.total(), page.page(), page.per_page());
            Response::ok(ListUsersResponse {
                users: page.items.into_iter().map(user_message).collect(),
                total,
                page: page_number,
                per_page,
                ..Default::default()
            })
        }
    }
}

fn user_message(user: UserRow) -> User {
    User {
        id: user.id.0,
        username: user.username,
        created_at: MessageField::some(user.created_at.into()),
        updated_at: MessageField::some(user.updated_at.into()),
        ..Default::default()
    }
}

fn auth_state(auth: AuthUser) -> AuthState {
    AuthState {
        user: MessageField::some(user_message(auth.user)),
        permissions: auth
            .permissions
            .iter()
            .map(|perm| perm.code().to_owned())
            .collect(),
        ..Default::default()
    }
}
