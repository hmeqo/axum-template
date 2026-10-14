use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator, IntoStaticStr};
use utoipa::ToSchema;

/// `PERM` 取同名 `Perm` 变体（`Require<P>` 据此判定）。
pub trait PermSpec {
    const PERM: Perm;
}

macro_rules! perms {
    ($($variant:ident = $code:literal, $description:literal;)*) => {
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            EnumString,
            EnumIter,
            IntoStaticStr,
            Serialize,
            Deserialize,
            Display,
            ToSchema,
        )]
        pub enum Perm {
            $(
                #[serde(rename = $code)]
                // `ToSchema` 读不到宏里插值的 `serde(rename)`，必须单独声明，否则文档里是变体名。
                #[schema(rename = $code)]
                #[strum(serialize = $code)]
                $variant,
            )*
        }

        impl Perm {
            pub const fn description(&self) -> &'static str {
                match self {
                    $(Self::$variant => $description,)*
                }
            }
        }

        $(
            pub struct $variant;

            impl PermSpec for $variant {
                const PERM: Perm = Perm::$variant;
            }
        )*
    };
}

perms! {
    All = "*", "Superuser";

    UserRead = "user:read", "View user information";
    UserWrite = "user:write", "Create or modify users";
    UserPasswordReset = "user:reset-password", "Reset another user's password";
    UserDelete = "user:delete", "Delete users";
    UserAll = "user:*", "All user-management permissions";

    RoleRead = "role:read", "View role information";
    RoleWrite = "role:write", "Create or modify roles";
    RoleDelete = "role:delete", "Delete roles";
    RoleAll = "role:*", "All role-management permissions";
}

pub fn perms_match(self_code: &str, target_code: &str) -> bool {
    if self_code == "*" {
        return true;
    }
    if self_code == target_code {
        return true;
    }
    if let Some(prefix) = self_code.strip_suffix(":*")
        && let Some(target_prefix) = target_code.split(':').next()
    {
        return prefix == target_prefix;
    }
    false
}

impl Perm {
    pub fn from_code(code: &str) -> Option<Self> {
        code.parse().ok()
    }

    pub fn code(&self) -> &'static str {
        self.into()
    }

    pub fn matches(&self, target_code: &str) -> bool {
        perms_match(self.code(), target_code)
    }

    pub fn all() -> Vec<Perm> {
        Self::iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator;

    use super::*;

    #[test]
    fn test_permissions_consistency() {
        for perm in Perm::iter() {
            let serde_json_str = serde_json::to_string(&perm).expect("Serialize failed");
            let serde_val = serde_json_str.trim_matches('"');

            let strum_val: &'static str = perm.into();

            assert_eq!(
                serde_val, strum_val,
                "JSON and Strum values for {} are inconsistent: {} != {}",
                perm, serde_val, strum_val
            );
        }
    }
}
