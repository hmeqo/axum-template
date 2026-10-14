//! 可信反向代理:解析 `server.trusted-proxies` 并据此决定 `X-Forwarded-For` 是否可用。

use std::net::IpAddr;

use ipnet::IpNet;

use crate::error::{AppError, Result};

/// 可信代理的地址集合;空集合表示永不采纳转发头。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustedProxies(Vec<IpNet>);

impl TrustedProxies {
    /// 条目是 IP 或 CIDR;任一条目无法解析即 `AppError::config`。
    pub fn new(entries: &[String]) -> Result<Self> {
        let mut nets = Vec::with_capacity(entries.len());
        for entry in entries {
            let trimmed = entry.trim();
            let net = trimmed
                .parse::<IpNet>()
                .ok()
                .or_else(|| trimmed.parse::<IpAddr>().ok().map(IpNet::from))
                .ok_or_else(|| {
                    AppError::config(format!(
                        "server.trusted-proxies entry is not an IP or CIDR: {entry}"
                    ))
                })?;
            nets.push(net);
        }
        Ok(Self(nets))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn contains(&self, ip: IpAddr) -> bool {
        self.0.iter().any(|net| net.contains(&ip))
    }

    /// 对端不可信(或集合为空)时整条转发头被忽略;可信时从右往左跳过可信跳,取第一个不可信值;
    /// 整条链都可信时取最左值;链为空或全是垃圾值时回落到对端地址。
    ///
    /// 地址统一走 `IpAddr::to_canonical()`,故 `::ffff:10.0.0.5` 与 `10.0.0.5` 同判。
    pub fn resolve(&self, peer: IpAddr, forwarded: Option<&str>) -> IpAddr {
        let peer = peer.to_canonical();
        if self.is_empty() || !self.contains(peer) {
            return peer;
        }

        let Some(forwarded) = forwarded else {
            return peer;
        };

        let chain: Vec<IpAddr> = forwarded
            .split(',')
            .filter_map(|value| value.trim().parse::<IpAddr>().ok())
            .collect();

        for ip in chain.iter().rev() {
            let ip = ip.to_canonical();
            if !self.contains(ip) {
                return ip;
            }
        }

        chain.first().map(|ip| ip.to_canonical()).unwrap_or(peer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trusted(entries: &[&str]) -> TrustedProxies {
        let entries: Vec<String> = entries.iter().map(|entry| (*entry).to_owned()).collect();
        TrustedProxies::new(&entries).unwrap()
    }

    fn ip(value: &str) -> IpAddr {
        value.parse().unwrap()
    }

    #[test]
    fn an_empty_set_never_trusts_a_forwarded_header() {
        let proxies = TrustedProxies::default();
        assert!(proxies.is_empty());
        assert_eq!(
            proxies.resolve(ip("10.0.0.5"), Some("203.0.113.7")),
            ip("10.0.0.5")
        );
    }

    #[test]
    fn an_untrusted_peer_is_the_client() {
        let proxies = trusted(&["10.0.0.0/8"]);
        assert_eq!(
            proxies.resolve(ip("127.0.0.1"), Some("203.0.113.7")),
            ip("127.0.0.1")
        );
    }

    #[test]
    fn the_rightmost_untrusted_hop_is_the_client() {
        let proxies = trusted(&["127.0.0.1", "10.0.0.0/8"]);
        assert_eq!(
            proxies.resolve(ip("127.0.0.1"), Some("203.0.113.7")),
            ip("203.0.113.7")
        );
        assert_eq!(
            proxies.resolve(ip("127.0.0.1"), Some("203.0.113.7, 10.0.0.5")),
            ip("203.0.113.7")
        );
    }

    #[test]
    fn an_all_trusted_chain_keeps_the_leftmost_value() {
        let proxies = trusted(&["127.0.0.1", "10.0.0.0/8"]);
        assert_eq!(
            proxies.resolve(ip("127.0.0.1"), Some("10.9.9.9, 10.0.0.5")),
            ip("10.9.9.9")
        );
    }

    #[test]
    fn a_v4_mapped_peer_matches_a_v4_range() {
        let proxies = trusted(&["10.0.0.0/8"]);
        assert_eq!(
            proxies.resolve(ip("::ffff:10.0.0.5"), Some("203.0.113.7")),
            ip("203.0.113.7")
        );
    }

    #[test]
    fn malformed_chain_entries_are_skipped() {
        let proxies = trusted(&["127.0.0.1"]);
        assert_eq!(
            proxies.resolve(ip("127.0.0.1"), Some("nonsense")),
            ip("127.0.0.1")
        );
        assert_eq!(proxies.resolve(ip("127.0.0.1"), Some("")), ip("127.0.0.1"));
        assert_eq!(proxies.resolve(ip("127.0.0.1"), None), ip("127.0.0.1"));
        assert_eq!(
            proxies.resolve(ip("127.0.0.1"), Some("garbage, 203.0.113.7")),
            ip("203.0.113.7")
        );
    }

    #[test]
    fn a_malformed_entry_is_a_config_error() {
        assert!(TrustedProxies::new(&["not-an-ip".to_owned()]).is_err());
        assert!(TrustedProxies::new(&["10.0.0.0/33".to_owned()]).is_err());
        assert!(!trusted(&["127.0.0.1", "::1", "10.0.0.0/8", "2001:db8::/32"]).is_empty());
    }
}
