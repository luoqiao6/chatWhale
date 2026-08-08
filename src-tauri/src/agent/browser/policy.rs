use crate::agent::types::BrowserContentPolicy;
use std::collections::HashMap;

pub fn host_of(url: &str) -> String {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let host_port = after_scheme.split(['/', '?', '#']).next().unwrap_or("");
    host_port.split(':').next().unwrap_or("").to_ascii_lowercase()
}

pub fn match_domain(host: &str, pattern: &str) -> bool {
    let host = host.to_ascii_lowercase();
    let pattern = pattern.to_ascii_lowercase();
    if let Some(rest) = pattern.strip_prefix("*.") {
        host.ends_with(&format!(".{rest}"))
    } else {
        host == pattern
    }
}

pub fn resolve_policy(
    host: &str,
    domain_policy: &HashMap<String, BrowserContentPolicy>,
    global: BrowserContentPolicy,
) -> BrowserContentPolicy {
    let mut best: Option<(usize, BrowserContentPolicy)> = None;
    for (pattern, level) in domain_policy {
        if match_domain(host, pattern) {
            let len = pattern.len();
            if best.map(|(bl, _)| len > bl).unwrap_or(true) {
                best = Some((len, *level));
            }
        }
    }
    best.map(|(_, l)| l).unwrap_or(global)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_host_from_url() {
        assert_eq!(host_of("https://example.com/a?b=1"), "example.com");
        assert_eq!(host_of("http://127.0.0.1:8080/x"), "127.0.0.1");
        assert_eq!(host_of("example.com"), "example.com");
    }

    #[test]
    fn matches_exact_and_wildcard_domain() {
        assert!(match_domain("example.com", "example.com"));
        assert!(!match_domain("sub.example.com", "example.com"));
        // 通配只匹配子域，不匹配 apex 本身（与 resolve_policy 的最长匹配测试一致）
        assert!(!match_domain("example.com", "*.example.com"));
        assert!(match_domain("a.b.example.com", "*.example.com"));
        assert!(!match_domain("example.org", "*.example.com"));
    }

    #[test]
    fn resolves_policy_with_longest_match() {
        let mut map = HashMap::new();
        map.insert("example.com".to_string(), BrowserContentPolicy::Normal);
        map.insert("*.example.com".to_string(), BrowserContentPolicy::Trusted);
        assert_eq!(
            resolve_policy("a.example.com", &map, BrowserContentPolicy::Strict),
            BrowserContentPolicy::Trusted
        );
        assert_eq!(
            resolve_policy("example.com", &map, BrowserContentPolicy::Strict),
            BrowserContentPolicy::Normal
        );
        assert_eq!(
            resolve_policy("other.org", &map, BrowserContentPolicy::Strict),
            BrowserContentPolicy::Strict
        );
    }
}
