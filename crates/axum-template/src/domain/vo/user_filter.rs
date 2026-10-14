//! 用户列表的过滤条件:用户名按空白切成词,每个词都必须出现(忽略大小写)。

use crate::error::{AppError, Result};

/// 搜索词上限;超过它的请求是 `400`。
pub const MAX_TERM_CHARS: usize = 32;

/// `ILIKE` 模式的转义字符:搜索词里的 `%`、`_` 与该字符本身按字面匹配。
pub const LIKE_ESCAPE: char = '\\';

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserFilter {
    username_terms: Vec<String>,
}

impl UserFilter {
    /// 空白搜索词等同不过滤;重复的词只留一个。
    pub fn new(username: Option<String>) -> Result<Self> {
        let Some(raw) = username else {
            return Ok(Self::default());
        };

        let raw = raw.trim();
        if raw.is_empty() {
            return Ok(Self::default());
        }
        if raw.chars().count() > MAX_TERM_CHARS {
            return Err(AppError::malformed_request(format!(
                "username search term must be at most {MAX_TERM_CHARS} characters"
            )));
        }

        let mut username_terms: Vec<String> = Vec::new();
        for term in raw.split_whitespace() {
            if !username_terms.iter().any(|seen| seen == term) {
                username_terms.push(term.to_owned());
            }
        }

        Ok(Self { username_terms })
    }

    /// 子串匹配的 `ILIKE` 模式(每个词前后各一个通配符、词内 `%`/`_`/`\` 已转义);空表示不过滤。
    pub fn username_patterns(&self) -> Vec<String> {
        self.username_terms
            .iter()
            .map(|term| {
                let mut pattern = String::with_capacity(term.len() + 2);
                pattern.push('%');
                for c in term.chars() {
                    if c == '%' || c == '_' || c == LIKE_ESCAPE {
                        pattern.push(LIKE_ESCAPE);
                    }
                    pattern.push(c);
                }
                pattern.push('%');
                pattern
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patterns(username: &str) -> Vec<String> {
        UserFilter::new(Some(username.to_owned()))
            .unwrap()
            .username_patterns()
    }

    #[test]
    fn a_blank_term_does_not_filter() {
        assert_eq!(UserFilter::new(None).unwrap(), UserFilter::default());
        assert_eq!(
            UserFilter::new(Some("   ".into())).unwrap(),
            UserFilter::default()
        );
        assert!(patterns("").is_empty());
    }

    #[test]
    fn every_word_must_appear_and_order_does_not_matter() {
        assert_eq!(patterns("Zhang San"), ["%Zhang%", "%San%"]);
        assert_eq!(patterns("  San   Zhang "), ["%San%", "%Zhang%"]);
        assert_eq!(patterns("San San"), ["%San%"], "repeated words collapse");
    }

    #[test]
    fn wildcards_in_a_word_are_matched_literally() {
        assert_eq!(patterns("50%_off\\"), ["%50\\%\\_off\\\\%"]);
    }

    #[test]
    fn a_term_longer_than_the_cap_is_rejected() {
        let exact = "a".repeat(MAX_TERM_CHARS);
        assert!(UserFilter::new(Some(exact)).is_ok());
        let long = "a".repeat(MAX_TERM_CHARS + 1);
        assert!(UserFilter::new(Some(long)).is_err());
    }
}
