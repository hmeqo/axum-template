use serde::Serialize;
use strum::EnumIter;
use utoipa::ToSchema;

macro_rules! error_codes {
    ($($variant:ident = $code:literal => $message:literal;)*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema, EnumIter)]
        pub enum ErrorCode {
            $(
                #[serde(rename = $code)]
                // `ToSchema` 读不到宏里插值的 `serde(rename)`，必须单独声明，否则文档里是变体名。
                #[schema(rename = $code)]
                $variant,
            )*
        }

        impl ErrorCode {
            pub fn message(self) -> &'static str {
                match self {
                    $(Self::$variant => $message,)*
                }
            }
        }
    };
}

error_codes! {
    InputInvalidFields = "input.invalid_fields" => "Validation failed";
    AuthInvalidCredentials = "auth.invalid_credentials" => "Invalid username or password";
    AuthSessionExpired = "auth.session_expired" => "Session expired";
    AuthRefreshTokenInvalid = "auth.refresh_token_invalid" => "Invalid refresh token";
    UserUsernameTaken = "user.username_taken" => "Username already exists";
    UserInvalidOldPassword = "user.invalid_old_password" => "Invalid old password";
    RoleNameTaken = "role.name_taken" => "Role already exists";
    RoleAlreadyAssigned = "role.already_assigned" => "Role already assigned to user";
    FileTooLarge = "file.too_large" => "File exceeds the configured size limit";
    RealtimeUnknownFrame = "realtime.unknown_frame" => "Unrecognized realtime frame";
    RealtimeTopicForbidden = "realtime.topic_forbidden" => "Not allowed to subscribe to this topic";
}

impl ErrorCode {
    /// 线上的 code 值；serde 的 `rename` 是它的唯一声明。
    pub(crate) fn wire_code(self) -> String {
        match serde_json::to_value(self) {
            Ok(serde_json::Value::String(code)) => code,
            other => unreachable!("ErrorCode serializes as a string: {other:?}"),
        }
    }
}
