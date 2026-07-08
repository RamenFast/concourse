// SPDX-License-Identifier: GPL-3.0-or-later
//! `concourse doctor` — the audit as a verb. The registry is honest because
//! this keeps re-checking it: convention conformance per node, branch
//! discipline per repo, governance wiring for the whole house.

use crate::probe::{self, State};
use crate::registry::{ProbeSpec, Registry};
use crate::util;
use serde::Serialize;

#[derive(Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Pass,
    Warn,
    Fail,
}

#[derive(Serialize, Clone)]
pub struct Finding {
    pub subject: String, // node id or "governance"/"skills"
    pub check: String,
    pub level: Level,
    pub note: String,
}

#[derive(Serialize)]
pub struct Report {
    pub pass: usize,
    pub warn: usize,
    pub fail: usize,
    pub findings: Vec<Finding>,
}

fn f(subject: &str, check: &str, level: Level, note: impl Into<String>) -> Finding {
    Finding {
        subject: subject.into(),
        check: check.into(),
        level,
        note: note.into(),
    }
}

pub fn run(reg: &Registry) -> Report {
    let mut findings = Vec::new();

    // ── per node ──
    let lives = probe::probe_all(&reg.nodes);
    for (node, live) in reg.nodes.iter().zip(lives.iter()) {
        match live.state {
            State::Ok | State::Present => findings.push(f(
                &node.id,
                "probe",
                Level::Pass,
                format!("{} ({})", live.state.label(), live.detail),
            )),
            State::Unavailable => findings.push(f(
                &node.id,
                "probe",
                Level::Warn,
                format!("unavailable — {}", live.detail),
            )),
            State::Unprobed => findings.push(f(
                &node.id,
                "probe",
                Level::Warn,
                "no probe declared — the registry can't verify this claim",
            )),
            State::Error | State::Missing => findings.push(f(
                &node.id,
                "probe",
                Level::Fail,
                live.detail.clone(),
            )),
        }

        // envelope conformance for cmd-probed tools that answered
        if let (Some(ProbeSpec::Cmd { .. }), Some(raw)) = (&node.probe, &live.raw) {
            for field in ["status", "tool", "version", "ts"] {
                if raw.get(field).is_none() {
                    // name the known drift kindly (rulings R1/R2)
                    let drift_hint = match field {
                        "ts" if raw.get("ts_ms").is_some() => " (has ts_ms — ruling R1)",
                        "version" if raw.get("proto_version").is_some() => {
                            " (has proto_version — ruling R2)"
                        }
                        _ => "",
                    };
                    findings.push(f(
                        &node.id,
                        "envelope",
                        Level::Warn,
                        format!("one-shot output missing `{field}`{drift_hint}"),
                    ));
                }
            }
        }

        // branch discipline (filing cabinet §4): ≤ 1 branch besides master/main
        if let Some(path) = &node.path {
            let dir = util::expand_home(path);
            if dir.join(".git").exists() {
                let out = util::run_with_timeout(
                    &[
                        "git".into(),
                        "-C".into(),
                        dir.to_string_lossy().into_owned(),
                        "branch".into(),
                        "--format=%(refname:short)".into(),
                    ],
                    4000,
                    None,
                );
                if out.exit == Some(0) {
                    let branches: Vec<&str> =
                        out.stdout.lines().filter(|l| !l.trim().is_empty()).collect();
                    let extras: Vec<&str> = branches
                        .iter()
                        .copied()
                        .filter(|b| *b != "master" && *b != "main")
                        .collect();
                    if extras.len() > 1 {
                        findings.push(f(
                            &node.id,
                            "branches",
                            Level::Warn,
                            format!(
                                "{} branches besides master/main ({}) — house law is at most one",
                                extras.len(),
                                extras.join(", ")
                            ),
                        ));
                    } else {
                        findings.push(f(
                            &node.id,
                            "branches",
                            Level::Pass,
                            format!("{} local branch(es)", branches.len()),
                        ));
                    }
                }
            }
        }

        // declared skills must exist
        if let Some(sk) = &node.skills {
            let c = crate::skills::claude_root().join(&sk.claude).join("SKILL.md");
            let h = crate::skills::hermes_root().join(&sk.hermes).join("SKILL.md");
            if !c.is_file() {
                findings.push(f(&node.id, "skill-claude", Level::Warn, format!("missing {}", c.display())));
            }
            if !h.is_file() {
                findings.push(f(&node.id, "skill-hermes", Level::Warn, format!("missing {}", h.display())));
            }
        }
    }

    // ── governance ──
    let cabinet = util::cabinet_path();
    if cabinet.is_file() {
        findings.push(f("governance", "cabinet-exists", Level::Pass, cabinet.display().to_string()));
        if let Ok(meta) = std::fs::metadata(&cabinet) {
            use std::os::unix::fs::PermissionsExt;
            let mode = meta.permissions().mode() & 0o777;
            if mode & 0o222 == 0 {
                findings.push(f("governance", "cabinet-readonly", Level::Pass, format!("mode {:o} — Ben's hand only", mode)));
            } else {
                findings.push(f(
                    "governance",
                    "cabinet-readonly",
                    Level::Warn,
                    format!("mode {:o} is writable — expected chmod 444 between Ben's edits", mode),
                ));
            }
        }
    } else {
        findings.push(f(
            "governance",
            "cabinet-exists",
            Level::Fail,
            "~/Dev/ClaudeWorkspace/AGENTS.md is missing — the filing cabinet is the house's law",
        ));
    }
    let claude_md = util::home().join(".claude/CLAUDE.md");
    let wired = std::fs::read_to_string(&claude_md)
        .map(|t| t.contains("AGENTS.md"))
        .unwrap_or(false);
    findings.push(if wired {
        f("governance", "claude-always-loads", Level::Pass, "~/.claude/CLAUDE.md imports the cabinet")
    } else {
        f(
            "governance",
            "claude-always-loads",
            Level::Warn,
            "~/.claude/CLAUDE.md does not reference the filing cabinet (AGENTS.md)",
        )
    });

    // ── skill mirrors ──
    let sk = crate::skills::report(&reg.skill_mirrors);
    for m in &sk.mirrors {
        findings.push(match m.state.as_str() {
            "paired" => f("skills", "mirror", Level::Pass, format!("{} ↔ {}", m.claude, m.hermes)),
            other => f(
                "skills",
                "mirror",
                Level::Warn,
                format!("{} ↔ {} — {}", m.claude, m.hermes, other),
            ),
        });
    }

    let pass = findings.iter().filter(|x| x.level == Level::Pass).count();
    let warn = findings.iter().filter(|x| x.level == Level::Warn).count();
    let fail = findings.iter().filter(|x| x.level == Level::Fail).count();
    Report { pass, warn, fail, findings }
}
