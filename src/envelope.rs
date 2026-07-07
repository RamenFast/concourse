// SPDX-License-Identifier: GPL-3.0-or-later
//! The convention envelope + exit codes. Every one-shot goes through here,
//! so concourse can't drift from the standard it audits.

use serde_json::{json, Map, Value};

pub const TOOL: &str = "concourse";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Exit codes per CONVENTION §4.
pub const EXIT_OK: i32 = 0;
pub const EXIT_UNAVAILABLE: i32 = 2;
pub const EXIT_BAD_ARGS: i32 = 3;
pub const EXIT_RUNTIME: i32 = 4;

/// Base envelope: {status, tool, version, ts} + payload fields.
pub fn ok(payload: Value) -> Value {
    let mut m = Map::new();
    m.insert("status".into(), json!("ok"));
    m.insert("tool".into(), json!(TOOL));
    m.insert("version".into(), json!(VERSION));
    m.insert("ts".into(), json!(crate::util::now_ts()));
    if let Value::Object(p) = payload {
        for (k, v) in p {
            m.insert(k, v);
        }
    }
    Value::Object(m)
}

pub fn err(error: &str, fix: &str) -> Value {
    json!({
        "status": "error",
        "tool": TOOL,
        "version": VERSION,
        "ts": crate::util::now_ts(),
        "error": error,
        "fix": fix,
    })
}

/// Emit a one-shot object on stdout (pretty; still one object) and exit.
pub fn emit_and_exit(v: Value, code: i32) -> ! {
    println!("{}", serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string()));
    std::process::exit(code);
}

/// Human-facing error (TTY path): one line to stdout, exit code.
pub fn human_err_exit(error: &str, fix: &str, code: i32) -> ! {
    println!("concourse: {error}");
    if !fix.is_empty() {
        println!("  fix: {fix}");
    }
    std::process::exit(code);
}

/// Decide JSON mode: --json forces; otherwise structured when piped.
pub fn json_mode(flag: bool) -> bool {
    flag || !crate::util::stdout_is_tty()
}
