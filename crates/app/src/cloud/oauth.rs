//! Google sign-in for installed apps: OAuth 2.0 with PKCE and a loopback
//! redirect (RFC 8252). The browser signs in, Google redirects to a one-shot
//! listener on 127.0.0.1, and the code is exchanged for an ID token, which
//! Kabar Cloud verifies.
//!
//! Used on the Mac, and on Android until the Credential Manager bridge lands.

use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
/// How long the browser has to come back.
const WAIT: Duration = Duration::from_secs(5 * 60);

/// A Google "Desktop app" OAuth client. Google treats the secret of an
/// installed app as non-confidential, but it still comes from the build
/// environment (`.env`), not the repo.
#[derive(Clone, Debug)]
pub struct GoogleClient {
    pub client_id: String,
    pub client_secret: Option<String>,
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum OAuthError {
    #[error("sign-in was cancelled")]
    Cancelled,
    #[error("sign-in timed out")]
    TimedOut,
    #[error("couldn't open the browser: {0}")]
    Browser(String),
    #[error("{0}")]
    Failed(String),
}

pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    pub fn new() -> Self {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).expect("system randomness");
        Self::from_verifier(URL_SAFE_NO_PAD.encode(bytes))
    }

    fn from_verifier(verifier: String) -> Self {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Self { verifier, challenge }
    }
}

fn random_state() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("system randomness");
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn auth_url(client_id: &str, redirect_uri: &str, pkce: &Pkce, state: &str) -> String {
    let params = [
        ("client_id", client_id),
        ("redirect_uri", redirect_uri),
        ("response_type", "code"),
        ("scope", "openid email"),
        ("code_challenge", &pkce.challenge),
        ("code_challenge_method", "S256"),
        ("state", state),
        ("prompt", "select_account"),
    ];
    format!("{AUTH_URL}?{}", form(&params))
}

/// Opens the browser, waits for Google's redirect and returns an ID token.
pub async fn google_id_token(client: &GoogleClient) -> Result<String, OAuthError> {
    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| OAuthError::Failed(e.to_string()))?;
    let port = listener.local_addr().map_err(|e| OAuthError::Failed(e.to_string()))?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}");
    let pkce = Pkce::new();
    let state = random_state();
    crate::platform::open_url(&auth_url(&client.client_id, &redirect_uri, &pkce, &state))
        .map_err(OAuthError::Browser)?;

    let code = tokio::time::timeout(WAIT, wait_for_code(&listener, &state))
        .await
        .map_err(|_| OAuthError::TimedOut)??;

    let client = client.clone();
    tokio::task::spawn_blocking(move || exchange(TOKEN_URL, &client, &code, &pkce.verifier, &redirect_uri))
        .await
        .map_err(|e| OAuthError::Failed(e.to_string()))?
}

async fn wait_for_code(listener: &TcpListener, state: &str) -> Result<String, OAuthError> {
    loop {
        let (mut stream, _) = listener.accept().await.map_err(|e| OAuthError::Failed(e.to_string()))?;
        let mut buf = vec![0u8; 8192];
        let n = stream.read(&mut buf).await.unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]);
        let Some(result) = parse_redirect(&request, state) else {
            // favicon.ico and other strays.
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\n\r\n").await;
            continue;
        };
        let page = if result.is_ok() { DONE_PAGE } else { CANCELLED_PAGE };
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{page}",
            page.len()
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.shutdown().await;
        return result;
    }
}

/// `GET /?code=…&state=… HTTP/1.1` → the code. `None` for requests that
/// aren't the redirect.
pub fn parse_redirect(request: &str, expected_state: &str) -> Option<Result<String, OAuthError>> {
    let target = request.lines().next()?.strip_prefix("GET ")?.split(' ').next()?;
    let query = target.strip_prefix("/?")?;
    let mut code = None;
    let mut state = None;
    let mut error = None;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let v = decode(v);
        match k {
            "code" => code = Some(v),
            "state" => state = Some(v),
            "error" => error = Some(v),
            _ => {}
        }
    }
    if state.as_deref() != Some(expected_state) {
        return Some(Err(OAuthError::Failed("state mismatch".into())));
    }
    Some(match (code, error) {
        (Some(code), None) => Ok(code),
        (_, Some(e)) if e == "access_denied" => Err(OAuthError::Cancelled),
        (_, Some(e)) => Err(OAuthError::Failed(e)),
        (None, None) => Err(OAuthError::Failed("no code".into())),
    })
}

#[derive(serde::Deserialize)]
struct TokenResponse {
    id_token: String,
}

fn exchange(
    token_url: &str,
    client: &GoogleClient,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<String, OAuthError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .into();
    let mut form = vec![
        ("code", code),
        ("client_id", client.client_id.as_str()),
        ("code_verifier", verifier),
        ("grant_type", "authorization_code"),
        ("redirect_uri", redirect_uri),
    ];
    if let Some(secret) = &client.client_secret {
        form.push(("client_secret", secret));
    }
    let mut res = agent.post(token_url).send_form(form).map_err(|e| OAuthError::Failed(e.to_string()))?;
    if !res.status().is_success() {
        let body = res.body_mut().read_to_string().unwrap_or_default();
        return Err(OAuthError::Failed(format!("token exchange {}: {body}", res.status())));
    }
    let token: TokenResponse = res.body_mut().read_json().map_err(|e| OAuthError::Failed(e.to_string()))?;
    Ok(token.id_token)
}

fn form(params: &[(&str, &str)]) -> String {
    params.iter().map(|(k, v)| format!("{}={}", encode(k), encode(v))).collect::<Vec<_>>().join("&")
}

fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn decode(s: &str) -> String {
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                (Some(h), Some(l)) => {
                    out.push(h << 4 | l);
                    i += 2;
                }
                _ => out.push(b'%'),
            },
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

const DONE_PAGE: &str = r#"<!doctype html><meta charset="utf-8"><title>Kabar</title>
<body style="font-family:system-ui;text-align:center;padding:3rem">
<h1>✓</h1><p>تمام، رجع لـ Kabar.</p><p>All set — go back to Kabar.</p></body>"#;

const CANCELLED_PAGE: &str = r#"<!doctype html><meta charset="utf-8"><title>Kabar</title>
<body style="font-family:system-ui;text-align:center;padding:3rem">
<p>متسجلتش. ارجع لـ Kabar وجرب تاني.</p><p>Not signed in. Go back to Kabar to try again.</p></body>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_matches_rfc_7636() {
        let p = Pkce::from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into());
        assert_eq!(p.challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        assert_eq!(Pkce::new().verifier.len(), 43);
    }

    #[test]
    fn auth_url_is_encoded() {
        let p = Pkce::from_verifier("v".into());
        let url = auth_url("123-desk.apps.googleusercontent.com", "http://127.0.0.1:5000", &p, "s1");
        assert!(url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?client_id=123-desk.apps"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A5000"));
        assert!(url.contains("scope=openid%20email"));
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn redirect_parsing() {
        let ok = "GET /?state=s1&code=4%2F0Ab_x&scope=email HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        assert_eq!(parse_redirect(ok, "s1"), Some(Ok("4/0Ab_x".into())));
        let denied = "GET /?error=access_denied&state=s1 HTTP/1.1\r\n";
        assert_eq!(parse_redirect(denied, "s1"), Some(Err(OAuthError::Cancelled)));
        let forged = "GET /?state=evil&code=x HTTP/1.1\r\n";
        assert!(matches!(parse_redirect(forged, "s1"), Some(Err(OAuthError::Failed(_)))));
        assert_eq!(parse_redirect("GET /favicon.ico HTTP/1.1\r\n", "s1"), None);
    }

    #[test]
    fn percent_decoding() {
        assert_eq!(decode("a%20b+c%2F"), "a b c/");
        assert_eq!(decode("100%"), "100%");
        assert_eq!(decode("%zz"), "%zz");
    }

    #[test]
    fn redirect_parsing_names_every_failure() {
        let other = "GET /?error=server_error&state=s1 HTTP/1.1\r\n";
        assert_eq!(parse_redirect(other, "s1"), Some(Err(OAuthError::Failed("server_error".into()))));
        let empty = "GET /?state=s1 HTTP/1.1\r\n";
        assert_eq!(parse_redirect(empty, "s1"), Some(Err(OAuthError::Failed("no code".into()))));
        assert_eq!(parse_redirect("POST /?code=x&state=s1 HTTP/1.1\r\n", "s1"), None);
        assert_eq!(
            OAuthError::Browser("no xdg-open".into()).to_string(),
            "couldn't open the browser: no xdg-open"
        );
        // Two sign-ins never share a state or a verifier.
        assert_ne!(random_state(), random_state());
        assert_ne!(Pkce::new().verifier, Pkce::new().verifier);
    }

    /// Sends `requests` to the loopback listener one connection each, and
    /// returns what the browser was shown for each.
    async fn browser(port: u16, requests: &[&str]) -> Vec<String> {
        let mut pages = Vec::new();
        for r in requests {
            let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            s.write_all(r.as_bytes()).await.unwrap();
            let mut page = String::new();
            s.read_to_string(&mut page).await.unwrap();
            pages.push(page);
        }
        pages
    }

    #[test]
    fn the_loopback_listener_skips_strays_and_thanks_the_browser() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let requests =
                ["GET /favicon.ico HTTP/1.1\r\n\r\n", "GET /?code=4%2F0Ab&state=s1 HTTP/1.1\r\n\r\n"];
            let (code, pages) = tokio::join!(wait_for_code(&listener, "s1"), browser(port, &requests));
            assert_eq!(code, Ok("4/0Ab".into()));
            assert!(pages[0].starts_with("HTTP/1.1 404"), "{}", pages[0]);
            assert!(
                pages[1].starts_with("HTTP/1.1 200") && pages[1].contains("go back to Kabar"),
                "{}",
                pages[1]
            );
            assert!(pages[1].contains("تمام، رجع لـ Kabar"));

            // "Cancel" on Google's page: the browser hears to try again.
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let requests = ["GET /?error=access_denied&state=s2 HTTP/1.1\r\n\r\n"];
            let (code, pages) = tokio::join!(wait_for_code(&listener, "s2"), browser(port, &requests));
            assert_eq!(code, Err(OAuthError::Cancelled));
            assert!(pages[0].contains("Not signed in"), "{}", pages[0]);
        });
    }

    /// A one-request token endpoint: answers `status` and `body`, and hands
    /// back the form it received.
    fn token_endpoint(status: u16, body: &'static str) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/token", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut got = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = s.read(&mut buf).unwrap();
                got.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&got).to_string();
                if let Some((head, rest)) = text.split_once("\r\n\r\n") {
                    let len = head
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if rest.len() >= len {
                        let reply = format!(
                            "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        s.write_all(reply.as_bytes()).unwrap();
                        return rest.to_string();
                    }
                }
            }
        });
        (url, handle)
    }

    #[test]
    fn the_code_is_exchanged_with_the_verifier_for_an_id_token() {
        let client = GoogleClient { client_id: "123-desk".into(), client_secret: Some("shh".into()) };
        let (url, sent) = token_endpoint(200, r#"{"id_token":"eyJhbGciOi.payload.sig","access_token":"x"}"#);
        let token = exchange(&url, &client, "4/0Ab", "verifier-1", "http://127.0.0.1:5000").unwrap();
        assert_eq!(token, "eyJhbGciOi.payload.sig");
        let form = sent.join().unwrap();
        for part in [
            "code=4%2F0Ab",
            "client_id=123-desk",
            "code_verifier=verifier-1",
            "grant_type=authorization_code",
            "client_secret=shh",
        ] {
            assert!(form.contains(part), "{part} missing from {form}");
        }

        // Google refuses the code: its answer is kept for the log.
        let public = GoogleClient { client_id: "123-desk".into(), client_secret: None };
        let (url, sent) = token_endpoint(400, r#"{"error":"invalid_grant"}"#);
        let err = exchange(&url, &public, "used", "v", "http://127.0.0.1:5000").unwrap_err();
        assert!(
            matches!(&err, OAuthError::Failed(m) if m.contains("400") && m.contains("invalid_grant")),
            "{err:?}"
        );
        assert!(!sent.join().unwrap().contains("client_secret"));

        // A 200 that isn't a token answer.
        let (url, _sent) = token_endpoint(200, "<html>captive portal</html>");
        assert!(matches!(
            exchange(&url, &public, "c", "v", "http://127.0.0.1:5000"),
            Err(OAuthError::Failed(_))
        ));
    }
}
