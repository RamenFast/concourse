// SPDX-License-Identifier: GPL-3.0-or-later
//! The registry: every node of the estate as data. Seeded from the repo copy,
//! living at ~/.config/concourse/registry.json (config is data — CONVENTION §6).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const SEED: &str = include_str!("../assets/registry.json");

#[derive(Deserialize, Serialize, Clone)]
pub struct Registry {
    pub version: u32,
    #[serde(default)]
    pub districts: Vec<String>,
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub skill_mirrors: Vec<Mirror>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Mirror {
    pub claude: String,
    pub hermes: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct SkillRefs {
    pub claude: String,
    pub hermes: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Node {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub glyph: String,
    pub kind: NodeKind,
    #[serde(default)]
    pub district: String,
    #[serde(default)]
    pub line: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probe: Option<ProbeSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remark_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open: Option<OpenSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signpost: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<SkillRefs>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verbs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Tool,
    Service,
    Place,
    Page,
}

impl NodeKind {
    pub fn label(self) -> &'static str {
        match self {
            NodeKind::Tool => "tool",
            NodeKind::Service => "service",
            NodeKind::Place => "place",
            NodeKind::Page => "page",
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ProbeSpec {
    Cmd {
        argv: Vec<String>,
        #[serde(default = "default_timeout")]
        timeout_ms: u64,
    },
    Http {
        url: String,
        #[serde(default = "default_timeout")]
        timeout_ms: u64,
    },
    File {
        path: String,
    },
    Dir {
        path: String,
    },
}

fn default_timeout() -> u64 {
    5000
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum OpenSpec {
    App {
        argv: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
    },
    Path {
        path: String,
    },
}

pub fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::util::home().join(".config"))
        .join("concourse")
}

pub fn registry_path() -> PathBuf {
    if let Some(p) = std::env::var_os("CONCOURSE_REGISTRY") {
        return PathBuf::from(p);
    }
    config_dir().join("registry.json")
}

/// Load the registry; first run installs the seed at the config path.
/// A broken config file is an error naming the fix, never a silent fallback.
pub fn load() -> Result<Registry, (String, String)> {
    let path = registry_path();
    if !path.exists() {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| (format!("cannot create {}: {e}", dir.display()), "check permissions on ~/.config".to_string()))?;
        }
        std::fs::write(&path, SEED)
            .map_err(|e| (format!("cannot seed registry at {}: {e}", path.display()), "check permissions".to_string()))?;
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|e| (format!("cannot read {}: {e}", path.display()), "check the file exists and is readable".to_string()))?;
    serde_json::from_str(&text).map_err(|e| {
        (
            format!("registry at {} is not valid: {e}", path.display()),
            format!("fix the JSON by hand, or remove the file to re-seed the default (a copy lives in the concourse repo at assets/registry.json)"),
        )
    })
}

impl Registry {
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
    pub fn ids(&self) -> Vec<&str> {
        self.nodes.iter().map(|n| n.id.as_str()).collect()
    }
}
