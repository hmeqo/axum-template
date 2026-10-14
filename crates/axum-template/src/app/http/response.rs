use std::borrow::Cow;

use axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use tokio_util::io::ReaderStream;

use crate::domain::{
    service::{StoredFile, thumbnail::is_thumbnailable},
    vo::{THUMB_PIPELINE, ThumbFormat, ThumbSize},
};

#[derive(Clone, Copy, strum::Display)]
enum CacheControl {
    #[strum(to_string = "public, max-age=31536000, immutable")]
    Year,
    #[strum(to_string = "no-store")]
    NoStore,
}

/// RFC 8187 §3.2 `attr-char`（obsoletes RFC 5987）：集合外的字节（含非 ASCII，按 UTF-8 逐字节）百分号编码。
const ATTR_CHAR: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'!')
    .remove(b'#')
    .remove(b'$')
    .remove(b'&')
    .remove(b'+')
    .remove(b'-')
    .remove(b'.')
    .remove(b'^')
    .remove(b'_')
    .remove(b'`')
    .remove(b'|')
    .remove(b'~');

/// 始终返回可见 ASCII 的 Content-Disposition 值；与回退名不同的原始名走 `filename*`（RFC 8187 §3.2）百分号编码。`HeaderValue::try_from` 必成功。
fn content_disposition(kind: &str, name: &str) -> HeaderValue {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let fallback: String = base
        .chars()
        .map(|c| {
            if c.is_ascii() && c.is_ascii_graphic() && !matches!(c, '"' | '\\') {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let fallback = if fallback.is_empty() {
        "unnamed".to_string()
    } else {
        fallback
    };

    let value = if !base.is_empty() && base == fallback {
        format!("{kind}; filename=\"{base}\"")
    } else if base.is_empty() {
        format!("{kind}; filename=\"{fallback}\"")
    } else {
        format!(
            "{kind}; filename=\"{fallback}\"; filename*=UTF-8''{}",
            utf8_percent_encode(base, ATTR_CHAR)
        )
    };
    HeaderValue::try_from(value).expect("sanitized Content-Disposition is valid ASCII")
}

const X_CONTENT_TYPE_OPTIONS: &str = "x-content-type-options";
const NOSNIFF: &str = "nosniff";
/// 派生图的响应依赖 `Accept`：同一 URL 会下发 AVIF 或 JPEG，必须声明。
const VARY_ACCEPT: &str = "Accept";

/// 原件的 disposition 由类型决定（栅格 `Inline`，其余 `Attachment`），派生图恒 `InlineDerived`。
// 非栅格不能内联：同源内联渲染用户上传物即存储型 XSS；栅格有 `nosniff` 加声明的栅格类型，浏览器只会当图片解码。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Attachment,
    Inline,
    /// 派生图：`inline`，不带 `filename`。
    InlineDerived,
}

/// 一份表示的下发策略：缓存指令、验证器、类型、长度与 disposition 一次定下。
pub struct Serving<'a> {
    cache: CacheControl,
    validator: Option<Cow<'a, str>>,
    content_type: &'a str,
    disposition: Disposition,
    len: u64,
    vary_accept: bool,
    stored: &'a StoredFile,
}

impl<'a> Serving<'a> {
    /// 原件的表示：类型与长度取行，验证器是行里的 sha256。
    pub fn of(stored: &'a StoredFile) -> Self {
        let (cache, validator) = if stored.state.is_attached() {
            (
                CacheControl::Year,
                Some(Cow::Borrowed(stored.file.hash.as_str())),
            )
        } else {
            (CacheControl::NoStore, None)
        };

        Self {
            cache,
            validator,
            content_type: &stored.file.mime_type,
            disposition: if is_thumbnailable(&stored.file.mime_type) {
                Disposition::Inline
            } else {
                Disposition::Attachment
            },
            len: stored.file.file_size as u64,
            vary_accept: false,
            stored,
        }
    }

    /// 派生图：`inline`；类型取编码格式；长度取派生文件；验证器带管线版本、档位与格式。
    pub fn thumbnail(
        stored: &'a StoredFile,
        size: ThumbSize,
        format: ThumbFormat,
        len: u64,
    ) -> Self {
        let (cache, validator) = if stored.state.is_attached() {
            (
                CacheControl::Year,
                Some(Cow::Owned(format!(
                    "{}-v{}-{}-{}",
                    stored.file.hash,
                    THUMB_PIPELINE,
                    size,
                    format.ext()
                ))),
            )
        } else {
            (CacheControl::NoStore, None)
        };

        Self {
            cache,
            validator,
            content_type: format.mime(),
            disposition: Disposition::InlineDerived,
            len,
            vary_accept: true,
            stored,
        }
    }

    /// 命中 `If-None-Match` 时的 `304`；没有验证器（draft）时恒为 `None`。
    pub fn not_modified(&self, headers: &HeaderMap) -> Option<Response> {
        let etag = self.validator.as_deref()?;
        if !if_none_match(headers, etag) {
            return None;
        }

        let mut builder = Response::builder()
            .status(StatusCode::NOT_MODIFIED)
            .header(header::ETAG, format!("\"{etag}\""))
            .header(header::CACHE_CONTROL, self.cache.to_string())
            .header(X_CONTENT_TYPE_OPTIONS, NOSNIFF);

        if self.vary_accept {
            builder = builder.header(header::VARY, VARY_ACCEPT);
        }

        Some(
            builder
                .body(Body::empty())
                .expect("response headers are static"),
        )
    }

    /// body 流式写出（内存 O(1)）。
    pub fn stream(&self, file: tokio::fs::File) -> Response {
        let stored = self.stored;
        // 入库的 mime 由上传方给出：放不进头就退回通用类型，而不是让响应构造失败。
        let content_type = HeaderValue::try_from(self.content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
        let disposition = match self.disposition {
            Disposition::Attachment => {
                content_disposition("attachment", &stored.file.original_name)
            }
            Disposition::Inline => content_disposition("inline", &stored.file.original_name),
            // 派生图不是原名指的那个文件，因此不带 filename：`inline` 即可让 `<img>` 直接渲染。
            Disposition::InlineDerived => HeaderValue::from_static("inline"),
        };

        let mut builder = Response::builder()
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CONTENT_DISPOSITION, disposition)
            .header(header::CACHE_CONTROL, self.cache.to_string())
            .header(header::CONTENT_LENGTH, self.len)
            .header(X_CONTENT_TYPE_OPTIONS, NOSNIFF);

        if let Some(etag) = self.validator.as_deref() {
            builder = builder.header(header::ETAG, format!("\"{etag}\""));
        }

        if self.vary_accept {
            builder = builder.header(header::VARY, VARY_ACCEPT);
        }

        builder
            .body(Body::from_stream(ReaderStream::new(file)))
            .expect("every response header is a validated value")
    }
}

/// 条件 GET 的 `If-None-Match` 比较：RFC 9110 弱比较（`W/` 前缀被忽略）；`*` 匹配任意表示。
fn if_none_match(headers: &HeaderMap, etag: &str) -> bool {
    let quoted = format!("\"{etag}\"");

    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value.split(',').any(|candidate| {
                let candidate = candidate.trim().trim_start_matches("W/");
                candidate == "*" || candidate == quoted
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn as_str(hv: HeaderValue) -> String {
        hv.to_str().expect("header is ASCII").to_string()
    }

    #[test]
    fn non_ascii_uses_rfc5987_and_stays_ascii() {
        let v = as_str(content_disposition("attachment", "期末报告.pdf"));
        assert!(v.is_ascii(), "must be visible ASCII: {v}");
        assert_eq!(
            v,
            "attachment; filename=\"____.pdf\"; \
             filename*=UTF-8''%E6%9C%9F%E6%9C%AB%E6%8A%A5%E5%91%8A.pdf"
        );
    }

    #[test]
    fn attr_chars_stay_literal_and_the_rest_is_pct_encoded() {
        // attr-char 原样，空格 / `?` / `%` 自身编码：`%` 必须变 `%25`，不得二次编码其他字节。
        let v = as_str(content_disposition("attachment", "r~p!t+v 1?%s.txt"));
        assert_eq!(
            v,
            "attachment; filename=\"r~p!t+v_1?%s.txt\"; \
             filename*=UTF-8''r~p!t+v%201%3F%25s.txt"
        );
    }

    #[test]
    fn strips_crlf_and_quotes() {
        let v = as_str(content_disposition("attachment", "a\r\nb.txt"));
        assert!(!v.contains('\r') && !v.contains('\n'), "no CR/LF: {v}");
        assert!(v.contains("filename=\"a__b.txt\""), "visible fallback: {v}");

        let v2 = as_str(content_disposition("attachment", "a\"b.txt"));
        assert!(v2.contains("filename=\"a_b.txt\""), "quotes replaced: {v2}");
    }

    #[test]
    fn strips_directory_components() {
        let v = as_str(content_disposition("attachment", "../a/../b.txt"));
        assert!(v.contains("filename=\"b.txt\""), "basename only: {v}");
    }

    #[test]
    fn empty_name_falls_back_to_unnamed() {
        let v = as_str(content_disposition("attachment", ""));
        assert!(v.contains("filename=\"unnamed\""));
    }

    fn request(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::IF_NONE_MATCH,
            HeaderValue::from_str(value).expect("a visible ASCII header value"),
        );
        headers
    }

    #[test]
    fn if_none_match_accepts_the_exact_weak_listed_and_star_forms() {
        assert!(if_none_match(&request("\"hash\""), "hash"));
        assert!(if_none_match(&request("W/\"hash\""), "hash"));
        assert!(if_none_match(&request("\"other\", \"hash\""), "hash"));
        assert!(if_none_match(&request("*"), "hash"));
    }

    #[test]
    fn if_none_match_ignores_absent_mismatching_and_unquoted_validators() {
        assert!(!if_none_match(&HeaderMap::new(), "hash"));
        assert!(!if_none_match(&request("\"other\""), "hash"));
        assert!(!if_none_match(&request("\"hash\""), "other"));
        assert!(
            !if_none_match(&request("hash"), "hash"),
            "the quotes are part of the validator"
        );
    }

    #[test]
    fn if_none_match_ignores_a_validator_that_is_not_visible_ascii() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::IF_NONE_MATCH,
            HeaderValue::from_bytes(b"\"/\xff\"").expect("obs-text is a valid header value"),
        );

        assert!(!if_none_match(&headers, "hash"));
    }
}
