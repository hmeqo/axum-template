use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

macro_rules! id_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Serialize,
            Deserialize,
            toasty::Embed,
            ToSchema,
        )]
        #[serde(transparent)]
        pub struct $name(pub i64);

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl From<i64> for $name {
            fn from(v: i64) -> Self {
                Self(v)
            }
        }

        impl validator::ValidateRange<i64> for $name {
            fn greater_than(&self, max: i64) -> Option<bool> {
                Some(self.0 > max)
            }

            fn less_than(&self, min: i64) -> Option<bool> {
                Some(self.0 < min)
            }
        }
    };
}

id_type! {
    UserId
}

id_type! {
    RoleId
}

id_type! {
    SessionId
}

id_type! {
    RefreshTokenId
}

/// File handle: an unguessable uuid v7 that is also the row's primary key.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    toasty::Embed,
    ToSchema,
)]
#[serde(transparent)]
#[schema(value_type = String, format = "uuid")]
pub struct FileId(Uuid);

impl FileId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for FileId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::str::FromStr for FileId {
    type Err = uuid::Error;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        raw.parse().map(Self)
    }
}

impl std::fmt::Display for FileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

id_type! {
    UserRoleId
}
