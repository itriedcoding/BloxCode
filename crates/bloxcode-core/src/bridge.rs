//! Loopback HTTP bridge consumed by the BloxCode Roblox Studio plugin.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Result};
use tiny_http::{Header, Method, Response, Server};

use crate::project::Snapshot;

const CORS_ORIGIN: (&str, &str) = ("Access-Control-Allow-Origin", "*");
const CORS_HEADERS: (&str, &str) = (
    "Access-Control-Allow-Headers",
    "Content-Type, X-BloxCode-Token, Authorization",
);
const CORS_METHODS: (&str, &str) = ("Access-Control-Allow-Methods", "GET, POST, OPTIONS");

#[derive(Debug, Default)]
pub struct BridgeStats {
    pub requests: AtomicU64,
    pub last_seen: Mutex<Option<u64>>,
}

impl BridgeStats {
    fn mark_seen(&self) {
        if let Ok(mut guard) = self.last_seen.lock() {
            *guard = Some(now_secs());
        }
    }

    /// Seconds since the Studio plugin last acknowledged, if ever.
    pub fn seconds_since_seen(&self) -> Option<u64> {
        let guard = self.last_seen.lock().ok()?;
        guard.map(|seen| now_secs().saturating_sub(seen))
    }

    pub fn request_count(&self) -> u64 {
        self.requests.load(Ordering::Relaxed)
    }
}

pub struct Bridge {
    pub port: u16,
    pub token: String,
    stop: Arc<AtomicBool>,
    stats: Arc<BridgeStats>,
    state: Arc<Mutex<Option<Snapshot>>>,
    thread: Option<JoinHandle<()>>,
}

impl Bridge {
    /// Bind the first free port at or after `preferred`.
    pub fn start(preferred: u16, token: String) -> Result<Self> {
        let mut last_error = String::new();
        for offset in 0..8u16 {
            let port = preferred.saturating_add(offset);
            if port < 1024 {
                continue;
            }
            match Server::http(("127.0.0.1", port)) {
                Ok(server) => {
                    let stop = Arc::new(AtomicBool::new(false));
                    let stats = Arc::new(BridgeStats::default());
                    let state: Arc<Mutex<Option<Snapshot>>> = Arc::new(Mutex::new(None));
                    let thread = {
                        let stop = Arc::clone(&stop);
                        let stats = Arc::clone(&stats);
                        let state = Arc::clone(&state);
                        let token = token.clone();
                        thread::Builder::new()
                            .name("bloxcode-bridge".into())
                            .spawn(move || serve(server, &stop, &stats, &state, &token))?
                    };
                    return Ok(Self {
                        port,
                        token,
                        stop,
                        stats,
                        state,
                        thread: Some(thread),
                    });
                }
                Err(error) => last_error = format!("port {port}: {error}"),
            }
        }
        bail!("could not bind the Studio bridge ({last_error})");
    }

    pub fn state(&self) -> Arc<Mutex<Option<Snapshot>>> {
        Arc::clone(&self.state)
    }

    pub fn stats(&self) -> Arc<BridgeStats> {
        Arc::clone(&self.stats)
    }

    pub fn publish(&self, snapshot: Option<Snapshot>) {
        if let Ok(mut guard) = self.state.lock() {
            *guard = snapshot;
        }
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

fn serve(
    server: Server,
    stop: &AtomicBool,
    stats: &BridgeStats,
    state: &Mutex<Option<Snapshot>>,
    token: &str,
) {
    while !stop.load(Ordering::Relaxed) {
        match server.recv_timeout(Duration::from_millis(250)) {
            Ok(Some(request)) => {
                stats.requests.fetch_add(1, Ordering::Relaxed);
                handle(request, stats, state, token);
            }
            Ok(None) => {}
            Err(_) => break,
        }
    }
}

fn handle(
    request: tiny_http::Request,
    stats: &BridgeStats,
    state: &Mutex<Option<Snapshot>>,
    token: &str,
) {
    if request.method() == &Method::Options {
        let _ = respond_raw(request, 204, String::new(), "text/plain");
        return;
    }

    let url = request.url().to_string();
    let (path, query) = match url.split_once('?') {
        Some((path, query)) => (path.to_string(), parse_query(query)),
        None => (url, Vec::new()),
    };

    let presented = query
        .iter()
        .find(|(key, _)| key == "token")
        .map(|(_, value)| value.clone())
        .or_else(|| header_value(&request, "X-BloxCode-Token"))
        .or_else(|| {
            header_value(&request, "Authorization")
                .and_then(|value| value.strip_prefix("Bearer ").map(|t| t.trim().to_string()))
        });

    if presented.as_deref() != Some(token) {
        let _ = respond_json(
            request,
            401,
            serde_json::json!({
                "error": "invalid or missing bridge token",
                "hint": "reinstall the BloxCode plugin from the Studio screen"
            }),
        );
        return;
    }

    stats.mark_seen();

    match path.as_str() {
        "/v1/health" => {
            let revision = state
                .lock()
                .ok()
                .and_then(|guard| guard.as_ref().map(|s| s.revision))
                .unwrap_or(0);
            let _ = respond_json(
                request,
                200,
                serde_json::json!({
                    "ok": true,
                    "app": "BloxCode",
                    "version": crate::VERSION,
                    "revision": revision,
                }),
            );
        }
        "/v1/manifest" => {
            let guard = state.lock().ok();
            let body = match guard.as_ref().and_then(|g| g.as_ref()) {
                Some(snapshot) => snapshot.manifest(),
                None => {
                    let _ = respond_json(
                        request,
                        409,
                        serde_json::json!({
                            "error": "no project is open in BloxCode"
                        }),
                    );
                    return;
                }
            };
            let _ = respond_json(request, 200, body);
        }
        "/v1/file" => {
            let requested = query
                .iter()
                .find(|(key, _)| key == "path")
                .map(|(_, value)| value.clone())
                .unwrap_or_default();
            let guard = state.lock().ok();
            let content = guard
                .as_ref()
                .and_then(|g| g.as_ref())
                .and_then(|snapshot| snapshot.source(&requested).cloned());
            match content {
                Some(content) => {
                    let _ = respond_raw(request, 200, content, "text/plain; charset=utf-8");
                }
                None => {
                    let _ = respond_json(
                        request,
                        404,
                        serde_json::json!({
                            "error": format!("unknown file `{requested}`")
                        }),
                    );
                }
            }
        }
        "/v1/studio/ack" => {
            stats.mark_seen();
            let _ = respond_json(request, 200, serde_json::json!({ "ok": true }));
        }
        _ => {
            let _ = respond_json(
                request,
                404,
                serde_json::json!({
                    "error": "unknown endpoint",
                    "endpoints": ["/v1/health", "/v1/manifest", "/v1/file?path=", "/v1/studio/ack"]
                }),
            );
        }
    }
}

fn header_value(request: &tiny_http::Request, name: &str) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|header| header.field.to_string().eq_ignore_ascii_case(name))
        .map(|header| header.value.as_str().to_string())
}

fn parse_query(raw: &str) -> Vec<(String, String)> {
    raw.split('&')
        .filter(|part| !part.is_empty())
        .map(|part| match part.split_once('=') {
            Some((key, value)) => (decode_percent(key), decode_percent(value)),
            None => (decode_percent(part), String::new()),
        })
        .collect()
}

fn decode_percent(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok();
            if let Some(value) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(value);
                index += 3;
                continue;
            }
        }
        if bytes[index] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[index]);
        }
        index += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn respond_json(
    request: tiny_http::Request,
    status: u16,
    value: serde_json::Value,
) -> std::io::Result<()> {
    let body = serde_json::to_string(&value).unwrap_or_else(|_| "{}".into());
    respond_raw(request, status, body, "application/json")
}

fn respond_raw(
    request: tiny_http::Request,
    status: u16,
    body: String,
    content_type: &str,
) -> std::io::Result<()> {
    let mut response = Response::from_string(body)
        .with_status_code(status)
        .with_header(header("Content-Type", content_type))
        .with_header(header(CORS_ORIGIN.0, CORS_ORIGIN.1))
        .with_header(header(CORS_HEADERS.0, CORS_HEADERS.1))
        .with_header(header(CORS_METHODS.0, CORS_METHODS.1))
        .with_header(header("Cache-Control", "no-store"));
    response.add_header(header("Connection", "close"));
    request.respond(response)
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes())
        .expect("static header name and value are valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_decoding_handles_paths() {
        let parsed = parse_query("token=abc&path=src%2Fshared%2FA.luau");
        assert_eq!(parsed[0], ("token".to_string(), "abc".to_string()));
        assert_eq!(
            parsed[1],
            ("path".to_string(), "src/shared/A.luau".to_string())
        );
    }

    #[test]
    fn bridge_rejects_bad_tokens_and_serves_manifest() {
        let bridge = Bridge::start(43_900, "test-token".to_string()).expect("bridge starts");
        let port = bridge.port;
        let snapshot = Snapshot {
            title: "Demo".into(),
            revision: 3,
            ..Default::default()
        };
        bridge.publish(Some(snapshot));

        let unauthorized = get(port, "/v1/manifest");
        assert!(unauthorized.starts_with("HTTP/1.1 401"), "{unauthorized}");

        let authorized = get(port, "/v1/manifest?token=test-token");
        assert!(authorized.starts_with("HTTP/1.1 200"), "{authorized}");
        assert!(authorized.contains("\"title\":\"Demo\""), "{authorized}");
        assert!(authorized.contains("\"revision\":3"), "{authorized}");
        assert_eq!(bridge.stats().seconds_since_seen(), Some(0));
    }

    fn get(port: u16, path: &str) -> String {
        use std::io::{Read, Write};
        use std::net::TcpStream;
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        let request =
            format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
        stream.write_all(request.as_bytes()).expect("write");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read");
        response
    }
}
