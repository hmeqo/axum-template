use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::{Validate, ValidationError};

use crate::{
    domain::{
        model::User,
        vo::{Page, Password, UserFilter, UserId, Username},
    },
    error::AppError,
};

#[derive(Debug, Default, Deserialize, Validate, ToSchema, IntoParams)]
#[serde(default)]
#[into_params(parameter_in = Query)]
pub struct UserFilterReq {
    // utoipa 的属性值只接受字面量,`#[param]` 这里必须再写一份数值;运行时的边界由
    // `#[validate]`(经 `MAX_TERM_CHARS`)决定,这一份只影响文档。
    /// Filters users whose username contains every whitespace-separated word, ignoring case; blank means no filter
    #[param(max_length = 32)]
    #[validate(custom(function = "username_search_rule"))]
    pub username: Option<String>,
}

impl TryFrom<UserFilterReq> for UserFilter {
    type Error = AppError;

    fn try_from(req: UserFilterReq) -> Result<Self, Self::Error> {
        UserFilter::new(req.username)
    }
}

fn username_search_rule(value: &str) -> Result<(), ValidationError> {
    field_rule("username", UserFilter::new(Some(value.to_owned())))
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateUserReq {
    #[validate(custom(function = "username_rule"))]
    pub username: String,
    #[validate(custom(function = "password_rule"))]
    pub password: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateUsernameReq {
    #[validate(custom(function = "username_rule"))]
    pub username: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ChangePasswordReq {
    pub old_password: String,
    #[validate(custom(function = "password_rule"))]
    pub new_password: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ResetPasswordReq {
    #[validate(custom(function = "password_rule"))]
    pub new_password: String,
}

fn username_rule(value: &str) -> Result<(), ValidationError> {
    field_rule("username", Username::new(value))
}

fn password_rule(value: &str) -> Result<(), ValidationError> {
    field_rule("password", Password::new(value))
}

fn field_rule<T>(field: &'static str, checked: Result<T, AppError>) -> Result<(), ValidationError> {
    checked
        .map(|_| ())
        .map_err(|e| ValidationError::new(field).with_message(e.to_string().into()))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserResp {
    pub id: UserId,
    pub username: String,
    #[schema(value_type = String, format = "date-time")]
    pub created_at: jiff::Timestamp,
    #[schema(value_type = String, format = "date-time")]
    pub updated_at: jiff::Timestamp,
}

impl From<User> for UserResp {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserListResp {
    pub users: Vec<UserResp>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

impl From<Page<UserResp>> for UserListResp {
    fn from(page: Page<UserResp>) -> Self {
        let (total, page_number, per_page) = (page.total(), page.page(), page.per_page());
        Self {
            users: page.items,
            total,
            page: page_number,
            per_page,
        }
    }
}
