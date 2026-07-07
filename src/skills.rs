// SPDX-License-Identifier: GPL-3.0-or-later
//! The skill estate: what Claude and Hermes each know, and which mirrors drift.
//! Claude skills: ~/.claude/skills/<name>/SKILL.md
//! Hermes skills: ~/.hermes/skills/<name>/SKILL.md (may nest one category level,
//! e.g. station/concourse). Mirrors sync only on Ben's manual ping — this only
//! *reports*.

use crate::registry::Mirror;
use crate::util;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone)]
pub struct SkillInfo {
    pub name: String, // path-ish id relative to the skills root ("station/phosphor")
    pub description: String,
    pub path: String,
}

#[derive(Serialize, Clone)]
pub struct MirrorState {
    pub claude: String,
    pub hermes: String,
    pub state: String, // "paired" | "claude-missing" | "hermes-missing" | "both-missing"
}

#[derive(Serialize, Clone)]
pub struct SkillsReport {
    pub claude_root: String,
    pub hermes_root: String,
    pub claude: Vec<SkillInfo>,
    pub hermes: Vec<SkillInfo>,
    pub mirrors: Vec<MirrorState>,
}

fn frontmatter_description(skill_md: &Path) -> String {
    let Ok(text) = std::fs::read_to_string(skill_md) else {
        return String::new();
    };
    // frontmatter `description:` first; else first heading line
    for line in text.lines().take(30) {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("description:") {
            return rest.trim().trim_matches('"').chars().take(140).collect();
        }
    }
    text.lines()
        .find(|l| l.trim_start().starts_with('#'))
        .map(|l| l.trim_start_matches(['#', ' ']).chars().take(140).collect())
        .unwrap_or_default()
}

fn scan_root(root: &Path, depth: usize) -> Vec<SkillInfo> {
    let mut out = Vec::new();
    walk(root, root, depth, &mut out);
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn walk(root: &Path, dir: &Path, depth_left: usize, out: &mut Vec<SkillInfo>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        let md = p.join("SKILL.md");
        if md.is_file() {
            let name = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .into_owned();
            out.push(SkillInfo {
                name,
                description: frontmatter_description(&md),
                path: md.to_string_lossy().into_owned(),
            });
        } else if depth_left > 0 {
            walk(root, &p, depth_left - 1, out);
        }
    }
}

pub fn claude_root() -> PathBuf {
    util::home().join(".claude/skills")
}
pub fn hermes_root() -> PathBuf {
    util::home().join(".hermes/skills")
}

pub fn report(mirrors: &[Mirror]) -> SkillsReport {
    let croot = claude_root();
    let hroot = hermes_root();
    let claude = scan_root(&croot, 1);
    let hermes = scan_root(&hroot, 1);
    let has = |list: &[SkillInfo], name: &str| list.iter().any(|s| s.name == name);
    let mirror_states = mirrors
        .iter()
        .map(|m| {
            let c = has(&claude, &m.claude);
            let h = has(&hermes, &m.hermes);
            MirrorState {
                claude: m.claude.clone(),
                hermes: m.hermes.clone(),
                state: match (c, h) {
                    (true, true) => "paired",
                    (true, false) => "hermes-missing",
                    (false, true) => "claude-missing",
                    (false, false) => "both-missing",
                }
                .into(),
            }
        })
        .collect();
    SkillsReport {
        claude_root: croot.to_string_lossy().into_owned(),
        hermes_root: hroot.to_string_lossy().into_owned(),
        claude,
        hermes,
        mirrors: mirror_states,
    }
}
