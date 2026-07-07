// SPDX-License-Identifier: GPL-3.0-or-later
//! The asks ledger: every unique ask, remembered. NDJSON, append-only,
//! at ~/.local/share/concourse/asks.ndjson. Agents and Ben both write it.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub struct Ask {
    pub ts: String,
    pub by: String,
    pub text: String,
}

pub fn ledger_path() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::util::home().join(".local/share"))
        .join("concourse/asks.ndjson")
}

pub fn list() -> Result<Vec<Ask>, String> {
    let path = ledger_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    Ok(text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect())
}

pub fn add(by: &str, text: &str) -> Result<Ask, String> {
    let ask = Ask {
        ts: crate::util::now_ts(),
        by: by.to_string(),
        text: text.trim().to_string(),
    };
    let path = ledger_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let line = serde_json::to_string(&ask).map_err(|e| e.to_string())?;
    writeln!(f, "{line}").map_err(|e| e.to_string())?;
    Ok(ask)
}
