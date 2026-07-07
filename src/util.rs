// SPDX-License-Identifier: GPL-3.0-or-later
//! Small shared plumbing: home expansion, timestamps, timeouts, loopback HTTP.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

/// Expand a leading `~` to $HOME. Anything else passes through.
pub fn expand_home(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        home().join(rest)
    } else if p == "~" {
        home()
    } else {
        PathBuf::from(p)
    }
}

/// ISO-8601 local timestamp with offset — the convention's `ts` (ruling R1).
pub fn now_ts() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

pub fn stdout_is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

pub struct CmdOutcome {
    pub exit: Option<i32>, // None = timed out (killed)
    pub stdout: String,
    pub stderr: String,
    pub elapsed_ms: u64,
    pub spawn_error: Option<String>,
}

/// Run argv with a wall-clock timeout. Never panics; spawn failure is data.
pub fn run_with_timeout(argv: &[String], timeout_ms: u64, cwd: Option<&PathBuf>) -> CmdOutcome {
    let started = Instant::now();
    let mut cmd = Command::new(expand_home(&argv[0]));
    cmd.args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return CmdOutcome {
                exit: None,
                stdout: String::new(),
                stderr: String::new(),
                elapsed_ms: started.elapsed().as_millis() as u64,
                spawn_error: Some(e.to_string()),
            }
        }
    };
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let exit;
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                exit = st.code();
                break;
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    exit = None;
                    break;
                }
                std::thread::sleep(Duration::from_millis(12));
            }
            Err(_) => {
                exit = None;
                break;
            }
        }
    }
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut stdout);
    }
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut stderr);
    }
    CmdOutcome {
        exit,
        stdout,
        stderr,
        elapsed_ms: started.elapsed().as_millis() as u64,
        spawn_error: None,
    }
}

/// Spawn detached (for `open` actions): new session-ish, stdio to null.
pub fn spawn_detached(argv: &[String], cwd: Option<&PathBuf>) -> Result<(), String> {
    let mut cmd = Command::new(expand_home(&argv[0]));
    cmd.args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

pub fn xdg_open(path: &PathBuf) -> Result<(), String> {
    spawn_detached(&["xdg-open".into(), path.to_string_lossy().into_owned()], None)
}

/// Minimal HTTP/1.1 GET for loopback probes only (the bind address is the
/// security boundary — R7). Returns (http_status, body).
pub fn http_get_loopback(url: &str, timeout_ms: u64) -> Result<(u16, String), String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("only http:// loopback urls are probed, got {url}"))?;
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let addr: std::net::SocketAddr = hostport
        .parse()
        .map_err(|_| format!("bad host:port in {url}"))?;
    let timeout = Duration::from_millis(timeout_ms);
    let mut stream = std::net::TcpStream::connect_timeout(&addr, timeout)
        .map_err(|e| format!("connect: {e}"))?;
    stream.set_read_timeout(Some(timeout)).ok();
    stream.set_write_timeout(Some(timeout)).ok();
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {hostport}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).map_err(|e| format!("write: {e}"))?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).map_err(|e| format!("read: {e}"))?;
    let text = String::from_utf8_lossy(&buf);
    let mut lines = text.splitn(2, "\r\n\r\n");
    let head = lines.next().unwrap_or("");
    let body = lines.next().unwrap_or("").to_string();
    let status: u16 = head
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    // tolerate chunked encoding crudely: strip hex size lines if present
    let body = if head.to_ascii_lowercase().contains("transfer-encoding: chunked") {
        body.lines()
            .filter(|l| !l.trim().is_empty() && u64::from_str_radix(l.trim(), 16).is_err())
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        body
    };
    Ok((status, body))
}

/// Walk a dotted key path ("server.state") through a JSON value.
pub fn json_path<'a>(v: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut cur = v;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    Some(cur)
}

/// Terse scalar rendering for board remarks.
pub fn scalar_str(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
