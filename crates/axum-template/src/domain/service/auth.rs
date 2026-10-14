use crate::{
    bail,
    domain::{
        model::User,
        service::{RoleService, SessionService, TokenService, UserService},
        vo::{ClientInfo, Password, Perm, UserId},
    },
    error::{AppError, Result},
};

pub struct AuthUser {
    pub user: User,
    pub permissions: Vec<Perm>,
}

impl AuthUser {
    pub fn new(user: User, permissions: Vec<Perm>) -> Self {
        Self { user, permissions }
    }
}

pub struct LoginOutcome {
    pub user: AuthUser,
    pub session_token: String,
    pub expires_at: jiff::Timestamp,
}

pub struct JwtLoginOutcome {
    pub user: AuthUser,
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Debug, Clone)]
pub struct AuthService {
    user: UserService,
    role: RoleService,
    session: SessionService,
    token: TokenService,
}

impl AuthService {
    pub fn new(
        user: UserService,
        role: RoleService,
        session: SessionService,
        token: TokenService,
    ) -> Self {
        Self {
            user,
            role,
            session,
            token,
        }
    }

    pub async fn authenticate(&self, username: &str, password: &str) -> Result<Option<AuthUser>> {
        let Some(user) = self.user.find_by_username(username).await? else {
            return Ok(None);
        };

        if !user.password.verify(password)? {
            return Ok(None);
        }

        let permissions = self.role.get_user_permissions(user.id).await?;
        Ok(Some(AuthUser::new(user, permissions)))
    }

    pub async fn get_auth_user(&self, user_id: UserId) -> Result<AuthUser> {
        let user = self.user.get_by_id(user_id).await?;
        let permissions = self.role.get_user_permissions(user_id).await?;
        Ok(AuthUser::new(user, permissions))
    }

    pub async fn check_permission(&self, user_id: UserId, perm: Perm) -> Result<bool> {
        let perms = self.role.get_user_permissions(user_id).await?;
        Ok(perms.iter().any(|p| p.matches(perm.code())))
    }

    pub async fn require_permission(&self, user_id: UserId, perm: Perm) -> Result<()> {
        if !self.check_permission(user_id, perm).await? {
            bail!(AppError::forbidden("Insufficient permissions"));
        }
        Ok(())
    }

    pub async fn login_session(
        &self,
        username: &str,
        password: &str,
        client: &ClientInfo,
    ) -> Result<Option<LoginOutcome>> {
        let Some(user) = self.authenticate(username, password).await? else {
            return Ok(None);
        };
        let (session_token, expires_at) = self.session.create(user.user.id, client).await?;
        Ok(Some(LoginOutcome {
            user,
            session_token,
            expires_at,
        }))
    }

    pub async fn login_jwt(
        &self,
        username: &str,
        password: &str,
        client: &ClientInfo,
    ) -> Result<Option<JwtLoginOutcome>> {
        let Some(user) = self.authenticate(username, password).await? else {
            return Ok(None);
        };
        let access_token = self.token.encode_access_token(&user.user)?;
        let refresh_token = self
            .token
            .generate_refresh_token(user.user.id, client)
            .await?;
        Ok(Some(JwtLoginOutcome {
            user,
            access_token,
            refresh_token,
        }))
    }

    pub async fn logout_all_sessions(&self, user_id: UserId) -> Result<()> {
        self.session.delete_by_user_id(user_id).await
    }

    pub async fn logout_all_refresh_tokens(&self, user_id: UserId) -> Result<()> {
        self.token.delete_all_refresh_tokens(user_id).await
    }

    pub async fn delete_user(&self, user_id: UserId) -> Result<()> {
        self.user.delete(user_id).await?;
        self.logout_all_sessions(user_id).await
    }

    /// 改密：改列 → 吊销该用户的全部会话与刷新令牌 → 调用方本来在会话上时，为它发一条新会话。
    ///
    /// 返回新会话的令牌与 `expires_at`，调用方据此重发 cookie；`None` = 调用方不在会话上。
    pub async fn change_password(
        &self,
        user_id: UserId,
        old_password: &str,
        new_password: Password,
        client: &ClientInfo,
        on_session: bool,
    ) -> Result<Option<(String, jiff::Timestamp)>> {
        self.user
            .change_password(user_id, old_password, new_password)
            .await?;

        // 不做事务：改列成功后吊销失败，会留下「密码已换、旧会话仍活」的窗口。
        self.logout_all_sessions(user_id).await?;
        // 刷新令牌没有「当前设备」的概念：改密后任何设备都要重新登录。
        self.logout_all_refresh_tokens(user_id).await?;

        // 轮换而非复用：旧令牌在改密这一刻作废，当前设备靠这条新会话继续登录。
        if on_session {
            return self.session.create(user_id, client).await.map(Some);
        }
        Ok(None)
    }

    pub async fn reset_password(&self, user_id: UserId, new_password: Password) -> Result<()> {
        self.user.reset_password(user_id, new_password).await?;
        self.logout_all_sessions(user_id).await?;
        self.logout_all_refresh_tokens(user_id).await
    }
}
