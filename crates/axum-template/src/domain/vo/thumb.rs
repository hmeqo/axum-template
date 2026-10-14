use std::fmt;

/// 允许请求的缩略图最长边。`inside` 语义：输出最长边不超过它，保比例、不裁、不放大。
pub const THUMB_SIZES: [ThumbSize; 5] = [
    ThumbSize(64),
    ThumbSize(128),
    ThumbSize(256),
    ThumbSize(512),
    // 「打开」用的查看档：1920 宽窗口里 1:1，原图超过它才需要它。
    ThumbSize(2048),
];

/// `FileResp::thumbnail_url` 使用的档位。
pub const THUMB_DEFAULT: ThumbSize = THUMB_SIZES[2];

/// 派生图管线版本：进文件名与 ETag。改编码参数（质量、速度、滤波器）时 +1，旧产物自然作废。
pub const THUMB_PIPELINE: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThumbSize(u32);

impl ThumbSize {
    /// 只接受白名单里的值；其余一律 `None`（调用方转 400）。
    pub fn parse(value: u32) -> Option<Self> {
        THUMB_SIZES.into_iter().find(|size| size.0 == value)
    }

    pub fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for ThumbSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbFormat {
    Avif,
    Jpeg,
}

/// `reconcile` 判定「期望的派生图集合」用。
pub const THUMB_FORMATS: [ThumbFormat; 2] = [ThumbFormat::Avif, ThumbFormat::Jpeg];

impl ThumbFormat {
    pub fn mime(self) -> &'static str {
        match self {
            Self::Avif => "image/avif",
            Self::Jpeg => "image/jpeg",
        }
    }

    /// 同时用作磁盘扩展名与 ETag 里的格式短码。
    pub fn ext(self) -> &'static str {
        match self {
            Self::Avif => "avif",
            Self::Jpeg => "jpg",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_only_the_canonical_sizes() {
        for size in THUMB_SIZES {
            assert_eq!(ThumbSize::parse(size.get()), Some(size));
        }

        for rejected in [0, 1, 63, 65, 128 + 1, 1000] {
            assert_eq!(ThumbSize::parse(rejected), None, "{rejected} is not a tier");
        }
    }

    #[test]
    fn display_matches_the_requested_number() {
        assert_eq!(THUMB_DEFAULT.to_string(), "256");
        assert_eq!(ThumbFormat::Avif.ext(), "avif");
        assert_eq!(ThumbFormat::Jpeg.ext(), "jpg");
        assert_eq!(ThumbFormat::Avif.mime(), "image/avif");
        assert_eq!(ThumbFormat::Jpeg.mime(), "image/jpeg");
    }
}
