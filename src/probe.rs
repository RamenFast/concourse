// SPDX-License-Identifier: GPL-3.0-or-later
//! Live probing: the registry claims, the probe checks. Claims carry their check.

use crate::registry::{Node, ProbeSpec};
use crate::util;
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Ok,          // probed, healthy
    Unavailable, // exists but can't serve right now (exit 2 / not listening)
    Error,       // tried and failed
    Missing,     // binary/file/dir not there
    Present,     // places/pages: exists (no deeper health to claim)
    Unprobed,    // no probe spec
}

impl State {
    pub fn label(self) -> &'static str {
        match self {
            State::Ok => "ok",
            State::Unavailable => "unavailable",
            State::Error => "error",
            State::Missing => "missing",
            State::Present => "present",
            State::Unprobed => "unprobed",
        }
    }
}

#[derive(Serialize, Clone)]
pub struct Live {
    pub id: String,
    pub state: State,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub detail: String,
    pub latency_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<Value>,
}

/// Probe one node, honestly. Never panics; every branch produces data.
pub fn probe(node: &Node) -> Live {
    let Some(spec) = &node.probe else {
        return Live {
            id: node.id.clone(),
            state: State::Unprobed,
            version: None,
            detail: "no probe declared".into(),
            latency_ms: 0,
            raw: None,
        };
    };
    match spec {
        ProbeSpec::File { path } => {
            let p = util::expand_home(path);
            let exists = p.is_file();
            Live {
                id: node.id.clone(),
                state: if exists { State::Present } else { State::Missing },
                version: None,
                detail: if exists {
                    "on disk".into()
                } else {
                    format!("{} is missing", p.display())
                },
                latency_ms: 0,
                raw: None,
            }
        }
        ProbeSpec::Dir { path } => {
            let p = util::expand_home(path);
            let exists = p.is_dir();
            Live {
                id: node.id.clone(),
                state: if exists { State::Present } else { State::Missing },
                version: None,
                detail: if exists {
                    "on disk".into()
                } else {
                    format!("{} is missing", p.display())
                },
                latency_ms: 0,
                raw: None,
            }
        }
        ProbeSpec::Http { url, timeout_ms } => match util::http_get_loopback(url, *timeout_ms) {
            Ok((code, body)) if (200..300).contains(&code) => {
                let raw: Option<Value> = serde_json::from_str(&body).ok();
                let version = raw
                    .as_ref()
                    .and_then(|v| v.get("version"))
                    .map(util::scalar_str);
                Live {
                    id: node.id.clone(),
                    state: State::Ok,
                    version,
                    detail: format!("http {code}"),
                    latency_ms: 0,
                    raw,
                }
            }
            Ok((code, _)) => Live {
                id: node.id.clone(),
                state: State::Error,
                version: None,
                detail: format!("http {code}"),
                latency_ms: 0,
                raw: None,
            },
            Err(e) => Live {
                id: node.id.clone(),
                state: State::Unavailable,
                version: None,
                detail: if e.starts_with("connect") {
                    "not listening".into()
                } else {
                    e
                },
                latency_ms: 0,
                raw: None,
            },
        },
        ProbeSpec::Cmd { argv, timeout_ms } => {
            let out = util::run_with_timeout(argv, *timeout_ms, None);
            if let Some(se) = out.spawn_error {
                return Live {
                    id: node.id.clone(),
                    state: State::Missing,
                    version: None,
                    detail: format!("{}: {se}", argv[0]),
                    latency_ms: out.elapsed_ms,
                    raw: None,
                };
            }
            let parsed: Option<Value> = serde_json::from_str(out.stdout.trim()).ok();
            match out.exit {
                Some(0) => match parsed {
                    Some(raw) => {
                        let version = raw.get("version").map(util::scalar_str);
                        let detail = remark(node, &raw);
                        Live {
                            id: node.id.clone(),
                            state: State::Ok,
                            version,
                            detail,
                            latency_ms: out.elapsed_ms,
                            raw: Some(raw),
                        }
                    }
                    None => Live {
                        id: node.id.clone(),
                        state: State::Error,
                        version: None,
                        detail: "exit 0 but stdout is not JSON (convention breach)".into(),
                        latency_ms: out.elapsed_ms,
                        raw: None,
                    },
                },
                Some(2) => {
                    let detail = parsed
                        .as_ref()
                        .and_then(|v| v.get("error"))
                        .map(util::scalar_str)
                        .unwrap_or_else(|| "unavailable (exit 2)".into());
                    Live {
                        id: node.id.clone(),
                        state: State::Unavailable,
                        version: parsed.as_ref().and_then(|v| v.get("version")).map(util::scalar_str),
                        detail,
                        latency_ms: out.elapsed_ms,
                        raw: parsed,
                    }
                }
                Some(code) => {
                    let detail = parsed
                        .as_ref()
                        .and_then(|v| v.get("error"))
                        .map(util::scalar_str)
                        .unwrap_or_else(|| {
                            let tail: String =
                                out.stderr.chars().rev().take(120).collect::<Vec<_>>().iter().rev().collect();
                            format!("exit {code}: {}", tail.trim())
                        });
                    Live {
                        id: node.id.clone(),
                        state: State::Error,
                        version: None,
                        detail,
                        latency_ms: out.elapsed_ms,
                        raw: parsed,
                    }
                }
                None => Live {
                    id: node.id.clone(),
                    state: State::Error,
                    version: None,
                    detail: format!("timed out after {timeout_ms} ms"),
                    latency_ms: out.elapsed_ms,
                    raw: None,
                },
            }
        }
    }
}

/// Build the board "remark" from declared remark_keys (dotted JSON paths).
fn remark(node: &Node, raw: &Value) -> String {
    if node.remark_keys.is_empty() {
        return "ok".into();
    }
    let parts: Vec<String> = node
        .remark_keys
        .iter()
        .filter_map(|k| {
            util::json_path(raw, k).map(|v| {
                let short = k.rsplit('.').next().unwrap_or(k);
                format!("{short} {}", util::scalar_str(v))
            })
        })
        .collect();
    if parts.is_empty() {
        "ok".into()
    } else {
        parts.join(" · ")
    }
}

/// Probe every node concurrently (bounded), preserving registry order.
pub fn probe_all(nodes: &[Node]) -> Vec<Live> {
    let mut handles = Vec::with_capacity(nodes.len());
    for node in nodes.iter().cloned() {
        handles.push(std::thread::spawn(move || probe(&node)));
    }
    handles
        .into_iter()
        .map(|h| {
            h.join().unwrap_or_else(|_| Live {
                id: "?".into(),
                state: State::Error,
                version: None,
                detail: "probe thread panicked".into(),
                latency_ms: 0,
                raw: None,
            })
        })
        .collect()
}
