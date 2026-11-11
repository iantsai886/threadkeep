//! `threadkeep app`: a small local server for the interface, used until the desktop shell
//! exists. It listens on 127.0.0.1 only, needs a per-launch key on every API call, and checks
//! the Host header so another website cannot reach it through the browser.
//! The route table is the same as the Node reference (docs/CONTRACTS.md section 6).

use serde_json::{json, Map, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;
use crate::home::random_hex;
use crate::ops::Ops;

/// (method, path) -> (owner command, query parameters it accepts)
const ROUTES: [(&str, &str, &str, &[&str]); 27] = [
    ("GET", "state", "state", &[]),
    ("GET", "doctor", "doctor", &[]),
    ("GET", "activity", "activity", &["n"]),
    ("GET", "overhead", "overhead", &[]),
    ("POST", "spaces", "addSpace", &[]),
    ("POST", "spaces/update", "updateSpace", &[]),
    ("POST", "spaces/remove", "removeSpace", &[]),
    ("GET", "connections", "connections", &[]),
    ("POST", "connections", "addConnection", &[]),
    ("POST", "connections/grants", "setGrants", &[]),
    ("POST", "connections/revoke", "revokeConnection", &[]),
    ("GET", "pending", "pending", &[]),
    ("POST", "pending/approve", "approve", &[]),
    ("POST", "pending/reject", "reject", &[]),
    ("POST", "ask", "ask", &[]),
    ("POST", "open", "open", &[]),
    ("POST", "preview", "preview", &[]),
    ("GET", "memories", "memories", &["space", "status"]),
    ("POST", "memories", "remember", &[]),
    ("POST", "memories/retire", "retire", &[]),
    ("POST", "memories/forget", "forget", &[]),
    ("GET", "threads", "threads", &["space", "all"]),
    ("POST", "threads/checkpoint", "checkpoint", &[]),
    ("GET", "backups", "listBackups", &["dir"]),
    ("POST", "backup", "backup", &[]),
    ("POST", "backup/verify", "verifyBackup", &[]),
    ("POST", "backup/restore", "restore", &[]),
];

const FALLBACK: &str = "<!doctype html><meta charset=\"utf-8\"><title>Threadkeep</title><body style=\"font-family:system-ui;max-width:40rem;margin:4rem auto;line-height:1.5\"><h1>Threadkeep is running</h1><p>The interface files were not found next to the program. Everything is available from the command line: run <code>threadkeep help</code>.</p>";
const HEADERS: &str = "Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n";
const CSP: &str = "Content-Security-Policy: default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; frame-ancestors 'none'\r\n";

/// The built interface: THREADKEEP_UI, or a `ui` folder beside the program, or the
/// repository's `ui/dist` when run from a development build.
pub fn find_ui() -> Option<PathBuf> {
    find_ui_in(&[])
}

/// Like `find_ui`, but also tries the given folders first (for example an app's bundled resources).
pub fn find_ui_in(extra: &[PathBuf]) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = extra.to_vec();
    candidates.extend(std::env::var("THREADKEEP_UI").ok().filter(|v| !v.is_empty()).map(PathBuf::from));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("ui"));
            candidates.push(dir.join("..").join("..").join("ui").join("dist"));
        }
    }
    candidates.into_iter().find(|c| c.join("index.html").is_file())
}

fn content_type(path: &Path) -> Option<&'static str> {
    Some(match path.extension()?.to_str()?.to_lowercase().as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        _ => return None,
    })
}

fn respond(stream: &mut TcpStream, status: &str, kind: &str, extra: &str, body: &[u8]) {
    let head = format!("HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n{HEADERS}{extra}\r\n", body.len());
    let _ = stream.write_all(head.as_bytes()).and_then(|_| stream.write_all(body)).and_then(|_| stream.flush());
}

fn send_json(stream: &mut TcpStream, status: &str, body: &Value) {
    respond(stream, status, "application/json", "", body.to_string().as_bytes());
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 3 <= bytes.len() && s.is_char_boundary(i + 1) && s.is_char_boundary(i + 3) => match u8::from_str_radix(&s[i + 1..i + 3], 16) {
                Ok(b) => {
                    out.push(b);
                    i += 3;
                    continue;
                }
                Err(_) => out.push(b'%'),
            },
            b'+' => out.push(b' '),
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn serve_static(stream: &mut TcpStream, ui: Option<&Path>, path: &str) {
    let rel = if path == "/" { "/index.html".to_string() } else { percent_decode(path) };
    let safe = rel.split('/').all(|p| p != ".." && !p.contains(['\\', ':']));
    let file = ui.filter(|_| safe).map(|u| rel.trim_start_matches('/').split('/').fold(u.to_path_buf(), |p, part| p.join(part)));
    match file.as_deref().and_then(|f| Some((content_type(f)?, std::fs::read(f).ok()?))) {
        Some((kind, body)) => respond(stream, "200 OK", kind, CSP, &body),
        None if rel == "/index.html" => respond(stream, "200 OK", "text/html; charset=utf-8", CSP, FALLBACK.as_bytes()),
        None => respond(stream, "404 Not Found", "text/plain", CSP, b""),
    }
}

fn handle(mut stream: TcpStream, ops: &Ops, port: u16, key: &str, ui: Option<&Path>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
    let Ok(clone) = stream.try_clone() else { return };
    let mut reader = BufReader::new(clone.take(1_100_000));
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    let mut parts = line.split_whitespace();
    let (method, target) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());
    let mut headers: Vec<(String, String)> = vec![];
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).unwrap_or(0) == 0 || h.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.push((k.trim().to_lowercase(), v.trim().to_string()));
        }
        if headers.len() > 100 {
            return;
        }
    }
    let header = |name: &str| headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str()).unwrap_or("");
    let host = header("host");
    if host != format!("127.0.0.1:{port}") && host != format!("localhost:{port}") {
        return respond(&mut stream, "421 Misdirected Request", "text/plain", "", b"");
    }
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    let Some(api) = path.strip_prefix("/api/") else {
        if method != "GET" {
            return respond(&mut stream, "405 Method Not Allowed", "text/plain", "", b"");
        }
        return serve_static(&mut stream, ui, path);
    };
    // The key travels in a header only: never in an API address, so it stays out of logs and history.
    if !constant_time_eq(header("x-threadkeep-token"), key) {
        return send_json(&mut stream, "401 Unauthorized", &json!({ "error": "This page has expired. Open Threadkeep again." }));
    }
    let Some((_, _, command, params)) = ROUTES.iter().find(|(m, p, _, _)| *m == method && *p == api) else {
        return send_json(&mut stream, "404 Not Found", &json!({ "error": "Unknown request." }));
    };
    let mut args = Value::Object(Map::new());
    if method == "POST" {
        if !header("content-type").starts_with("application/json") {
            return send_json(&mut stream, "415 Unsupported Media Type", &json!({ "error": "Send JSON." }));
        }
        let length: usize = header("content-length").parse().unwrap_or(0);
        if length > 1_000_000 {
            return send_json(&mut stream, "413 Payload Too Large", &json!({ "error": "Request too large." }));
        }
        let mut body = vec![0u8; length];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
        match if body.is_empty() { Ok(json!({})) } else { serde_json::from_slice::<Value>(&body) } {
            Ok(v) if v.is_object() => args = v,
            Ok(_) => return send_json(&mut stream, "400 Bad Request", &json!({ "error": "Send a JSON object." })),
            Err(_) => return send_json(&mut stream, "400 Bad Request", &json!({ "error": "That was not valid JSON." })),
        }
    } else {
        for pair in query.split('&').filter(|p| !p.is_empty()) {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            let (k, v) = (percent_decode(k), percent_decode(v));
            if !params.contains(&k.as_str()) || v.is_empty() {
                continue;
            }
            args[&k] = match k.as_str() {
                "all" => json!(v == "1"),
                "n" => json!(v.parse::<u64>().unwrap_or(30)),
                _ => json!(v),
            };
        }
    }
    match ops.dispatch(command, &args) {
        Ok(result) => send_json(&mut stream, "200 OK", &json!({ "ok": true, "result": result })),
        Err(e) => send_json(&mut stream, "400 Bad Request", &json!({ "ok": false, "error": e.0 })),
    }
}

pub struct App {
    pub listener: TcpListener,
    pub port: u16,
    pub key: String,
    pub url: String,
}

pub fn start(port: u16) -> std::io::Result<App> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let port = listener.local_addr()?.port();
    let key = random_hex(24);
    // The key reaches the page once, in the address fragment, which browsers never send to a server.
    let url = format!("http://127.0.0.1:{port}/#t={key}");
    Ok(App { listener, port, key, url })
}

/// Serves requests one at a time: every command reads and writes whole files, so there is
/// nothing to gain from handling two at once and no chance of two writes interleaving.
pub fn run(app: &App, ops: &Ops, ui: Option<&Path>) {
    for stream in app.listener.incoming().flatten() {
        handle(stream, ops, app.port, &app.key, ui);
    }
}
