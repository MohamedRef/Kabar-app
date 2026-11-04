//! The Kabar web app (`web/`, docs/WEB.md) talks to Kabar Cloud from a
//! browser: its origin gets CORS, and since a browser can't put an
//! `Authorization` header on a WebSocket, `/v1/sync` also takes the access
//! token as a WebSocket subprotocol (`kabar.v1, kabar.bearer.<token>`).
//!
//! CORS here guards nothing secret (the API takes bearer tokens, never
//! cookies); it only lets the web app's own pages read the answers.

pub use kabar_protocol::sync::{WS_BEARER_PREFIX, WS_PROTOCOL};

/// Worker → Durable Object: the subprotocol the socket answers with.
pub const PROTOCOL_HEADER: &str = "x-kabar-ws-protocol";

/// Headers a browser may send cross-origin.
pub const ALLOW_HEADERS: &str = "authorization, content-type";
pub const ALLOW_METHODS: &str = "GET, POST, PUT, DELETE, OPTIONS";
/// Browsers cache a preflight this long (seconds).
pub const MAX_AGE: &str = "86400";

/// Whether `origin` is a Kabar web app: the `kabar-web` Worker on any
/// workers.dev account, a local `just web`, or one listed in `WEB_ORIGINS`.
pub fn allowed_origin(origin: &str, extra: &[String]) -> bool {
    let origin = origin.trim_end_matches('/');
    if extra.iter().any(|o| o.trim_end_matches('/').eq_ignore_ascii_case(origin)) {
        return true;
    }
    if let Some(host) = origin.strip_prefix("https://kabar-web.")
        && let Some(account) = host.strip_suffix(".workers.dev")
    {
        return !account.is_empty() && account.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    }
    matches!(origin, "http://localhost:8080" | "http://127.0.0.1:8080")
}

/// The access token a browser offered in `Sec-WebSocket-Protocol`, when it
/// also offered [`WS_PROTOCOL`] (which the server then picks).
pub fn ws_bearer(protocols: &str) -> Option<String> {
    let offered: Vec<&str> = protocols.split(',').map(str::trim).collect();
    if !offered.contains(&WS_PROTOCOL) {
        return None;
    }
    offered.iter().find_map(|p| p.strip_prefix(WS_BEARER_PREFIX)).filter(|t| !t.is_empty()).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_kabar_web_origins_are_allowed() {
        assert!(allowed_origin("https://kabar-web.hamorefaat85.workers.dev", &[]));
        assert!(allowed_origin("http://localhost:8080", &[]));
        assert!(allowed_origin("http://127.0.0.1:8080/", &[]));
        assert!(!allowed_origin("https://kabar-web..workers.dev", &[]));
        assert!(!allowed_origin("https://kabar-web.evil.com", &[]));
        assert!(!allowed_origin("https://kabar-web.a.workers.dev.evil.com", &[]));
        assert!(!allowed_origin("https://kabar-web.a/b.workers.dev", &[]));
        assert!(!allowed_origin("http://kabar-web.a.workers.dev", &[]));
        assert!(!allowed_origin("https://example.com", &[]));
        assert!(allowed_origin("https://kabar.example.com", &["https://kabar.example.com".into()]));
    }

    #[test]
    fn the_socket_token_rides_in_the_subprotocol() {
        assert_eq!(ws_bearer("kabar.v1, kabar.bearer.abc.def-ghi_j").as_deref(), Some("abc.def-ghi_j"));
        assert_eq!(ws_bearer("kabar.bearer.abc"), None, "needs kabar.v1 too");
        assert_eq!(ws_bearer("kabar.v1"), None);
        assert_eq!(ws_bearer("kabar.v1, kabar.bearer."), None);
    }
}
