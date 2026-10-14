use serde::{Deserialize, Serialize};
use strum::{EnumIter, EnumString, IntoStaticStr};
use utoipa::ToSchema;

use crate::error::{AppError, Result};

/// 文件生命周期：`Draft` 未采用、可被 GC 回收；`Attached` 已被业务采用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileState {
    Draft,
    Attached { at: jiff::Timestamp },
}

/// Projection of [`FileState`] for the wire and the `files.state` column.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    ToSchema,
    IntoStaticStr,
    EnumString,
    EnumIter,
    toasty::Embed,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[column(rename_all = "lowercase", type = text)]
pub enum FileStateCode {
    Draft,
    Attached,
}

impl FileStateCode {
    pub fn as_str(self) -> &'static str {
        self.into()
    }
}

impl FileState {
    pub fn code(&self) -> FileStateCode {
        match self {
            Self::Draft => FileStateCode::Draft,
            Self::Attached { .. } => FileStateCode::Attached,
        }
    }

    pub fn attached_at(&self) -> Option<jiff::Timestamp> {
        match self {
            Self::Draft => None,
            Self::Attached { at } => Some(*at),
        }
    }

    pub fn is_attached(&self) -> bool {
        matches!(self, Self::Attached { .. })
    }

    /// 非法组合即内部错误。
    pub fn from_columns(code: FileStateCode, attached_at: Option<jiff::Timestamp>) -> Result<Self> {
        match (code, attached_at) {
            (FileStateCode::Draft, None) => Ok(Self::Draft),
            (FileStateCode::Attached, Some(at)) => Ok(Self::Attached { at }),
            (code, attached_at) => Err(AppError::internal(format!(
                "inconsistent file state: {code:?} / {attached_at:?}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: jiff::Timestamp = jiff::Timestamp::UNIX_EPOCH;

    #[test]
    fn codes_match_the_wire_text() {
        // wire 文本由 serde 的 rename 决定；列文本由 toasty embed 的 rename_all 决定。
        assert_eq!(FileStateCode::Draft.as_str(), "draft");
        assert_eq!(FileStateCode::Attached.as_str(), "attached");
        assert_eq!(
            "draft".parse::<FileStateCode>().unwrap(),
            FileStateCode::Draft
        );
    }

    #[test]
    fn draft_carries_no_timestamp_and_attached_carries_one() {
        assert_eq!(
            FileState::from_columns(FileStateCode::Draft, None).unwrap(),
            FileState::Draft
        );
        assert_eq!(
            FileState::from_columns(FileStateCode::Attached, Some(NOW)).unwrap(),
            FileState::Attached { at: NOW }
        );
    }

    #[test]
    fn inconsistent_columns_are_rejected() {
        for (code, at) in [
            (FileStateCode::Draft, Some(NOW)),
            (FileStateCode::Attached, None),
        ] {
            assert!(
                FileState::from_columns(code, at).is_err(),
                "{code:?} / {at:?} must be rejected"
            );
        }
    }
}
