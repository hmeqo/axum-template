use serde::Deserialize;
use utoipa::ToSchema;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateUserRequest {
    #[validate(length(
        min = 3,
        max = 20,
        message = "Username must be between 3 and 20 characters"
    ))]
    pub username: String,
    #[validate(length(
        min = 8,
        max = 32,
        message = "Password must be between 8 and 32 characters"
    ))]
    pub password: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateUsernameRequest {
    pub username: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ResetPasswordRequest {
    pub new_password: String,
}
