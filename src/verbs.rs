// SPDX-License-Identifier: GPL-3.0-or-later
//! CLI verbs: the agent surface and the human surface are the same verbs over
//! the same core — pretty on a TTY, envelope in a pipe, `--json` forces.

use crate::envelope::{self, emit_and_exit, EXIT_BAD_ARGS, EXIT_OK, EXIT_RUNTIME, EXIT_UNAVAILABLE};
use crate::probe::{self, State};
use crate::registry::{Node, OpenSpec, Registry};
use crate::util;
use serde_json::json;

// ── tiny ANSI kit (TTY only, honors NO_COLOR) ──────────────────────────────
fn color_on() -> bool {
    util::stdout_is_tty() && std::env::var_os("NO_COLOR").is_none()
}
fn paint(code: &str, s: &str) -> String {
    if color_on() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}
fn lamp(state: State) -> String {
    // semantic colors shared with the estate: ok green, amber unavailable,
    // red error, dim the rest (same meanings as surveyor's badges)
    match state {
        State::Ok => paint("38;5;114", "●"),
        State::Present => paint("38;5;108", "◆"),
        State::Unavailable => paint("38;5;214", "◐"),
        State::Error => paint("38;5;167", "✗"),
        State::Missing => paint("38;5;167", "▢"),
        State::Unprobed => paint("38;5;245", "·"),
    }
}
fn dim(s: &str) -> String {
    paint("38;5;245", s)
}
fn strong(s: &str) -> String {
    paint("1", s)
}

// ── shared bits ─────────────────────────────────────────────────────────────
fn disk_free_bytes() -> Option<u64> {
    let out = util::run_with_timeout(
        &["df".into(), "--output=avail".into(), "-B1".into(), "/".into()],
        3000,
        None,
    );
    out.stdout.lines().last()?.trim().parse().ok()
}

pub fn governance_summary() -> serde_json::Value {
    let path = util::cabinet_path();
    let present = path.is_file();
    let mut readonly = false;
    let mut sha12 = String::new();
    if present {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&path) {
            readonly = meta.permissions().mode() & 0o222 == 0;
        }
        if let Ok(bytes) = std::fs::read(&path) {
            use sha2::{Digest, Sha256};
            let d = Sha256::digest(&bytes);
            sha12 = format!("{d:x}")[..12].to_string();
        }
    }
    json!({
        "cabinet_path": path.to_string_lossy(),
        "present": present,
        "readonly": readonly,
        "sha256_12": sha12,
    })
}

fn find_node<'a>(reg: &'a Registry, id: &str) -> Result<&'a Node, (String, String)> {
    reg.node(id).ok_or_else(|| {
        let ids = reg.ids().join(", ");
        (
            format!("no node named `{id}`"),
            format!("one of: {ids} — or `concourse nodes` for the registry"),
        )
    })
}

// ── status ──────────────────────────────────────────────────────────────────
pub fn cmd_status(reg: &Registry, json: bool) -> ! {
    let started = std::time::Instant::now();
    let lives = probe::probe_all(&reg.nodes);
    let sk = crate::skills::report(&reg.skill_mirrors);
    let paired = sk.mirrors.iter().filter(|m| m.state == "paired").count();
    if json {
        let v = envelope::ok(json!({
            "generated_in_ms": started.elapsed().as_millis() as u64,
            "nodes": lives.iter().map(|l| json!({
                "id": l.id, "state": l.state.label(), "version": l.version,
                "detail": l.detail, "latency_ms": l.latency_ms,
            })).collect::<Vec<_>>(),
            "governance": governance_summary(),
            "skills": { "claude": sk.claude.len(), "hermes": sk.hermes.len(),
                         "mirrors_paired": paired, "mirrors_total": sk.mirrors.len() },
            "disk_free_bytes": disk_free_bytes(),
        }));
        emit_and_exit(v, EXIT_OK);
    }
    // human board
    println!(
        "{}   {}",
        strong("CONCOURSE — the station board"),
        dim(&util::now_ts())
    );
    println!("{}", dim(&"─".repeat(74)));
    let width = reg.nodes.iter().map(|n| n.name.len()).max().unwrap_or(12).min(28);
    for (node, live) in reg.nodes.iter().zip(lives.iter()) {
        let ver = live.version.clone().unwrap_or_else(|| "—".into());
        println!(
            " {} {:<w$}  {:<8} {}  {}",
            lamp(live.state),
            node.name.chars().take(w28(width)).collect::<String>(),
            ver.chars().take(8).collect::<String>(),
            format!("{:<40}", live.detail.chars().take(40).collect::<String>()),
            dim(&format!("{:>5}ms", live.latency_ms)),
            w = w28(width),
        );
    }
    println!("{}", dim(&"─".repeat(74)));
    let gov = governance_summary();
    let gov_line = if gov["present"].as_bool().unwrap_or(false) {
        format!(
            "cabinet {} · {}",
            if gov["readonly"].as_bool().unwrap_or(false) { "sealed (444)" } else { "WRITABLE" },
            gov["sha256_12"].as_str().unwrap_or("")
        )
    } else {
        "cabinet MISSING".into()
    };
    let disk = disk_free_bytes()
        .map(|b| format!("{:.1} GB free", b as f64 / 1e9))
        .unwrap_or_else(|| "disk ?".into());
    println!(
        " {}   {}   skills {}·{} ({} paired)",
        dim(&gov_line),
        dim(&disk),
        sk.claude.len(),
        sk.hermes.len(),
        paired
    );
    std::process::exit(EXIT_OK);
}

fn w28(w: usize) -> usize {
    w.clamp(12, 28)
}

// ── nodes / node / probe ────────────────────────────────────────────────────
pub fn cmd_nodes(reg: &Registry, json: bool) -> ! {
    if json {
        let v = envelope::ok(json!({
            "districts": reg.districts,
            "nodes": serde_json::to_value(&reg.nodes).unwrap_or(json!([])),
        }));
        emit_and_exit(v, EXIT_OK);
    }
    for d in &reg.districts {
        println!("{}", strong(d));
        for n in reg.nodes.iter().filter(|n| &n.district == d) {
            println!(
                "  {} {:<24} {}",
                n.glyph,
                n.id,
                dim(&n.line.chars().take(58).collect::<String>())
            );
        }
    }
    std::process::exit(EXIT_OK);
}

pub fn cmd_node(reg: &Registry, id: &str, json: bool) -> ! {
    match find_node(reg, id) {
        Err((e, fix)) => bad_args(&e, &fix, json),
        Ok(node) => {
            let live = probe::probe(node);
            if json {
                let v = envelope::ok(json!({
                    "node": serde_json::to_value(node).unwrap_or(json!({})),
                    "live": serde_json::to_value(&live).unwrap_or(json!({})),
                }));
                emit_and_exit(v, EXIT_OK);
            }
            println!("{} {}  {}", node.glyph, strong(&node.name), dim(&format!("[{}]", node.kind.label())));
            println!("  {}", node.line);
            if let Some(p) = &node.path {
                println!("  path      {p}");
            }
            if let Some(b) = &node.bin {
                println!("  bin       {b}");
            }
            if !node.verbs.is_empty() {
                println!("  verbs     {}", node.verbs.join(" · "));
            }
            if let Some(n) = &node.note {
                println!("  note      {n}");
            }
            println!(
                "  live      {} {}  {}",
                lamp(live.state),
                live.state.label(),
                dim(&live.detail)
            );
            std::process::exit(EXIT_OK);
        }
    }
}

pub fn cmd_probe(reg: &Registry, id: &str, json: bool) -> ! {
    match find_node(reg, id) {
        Err((e, fix)) => bad_args(&e, &fix, json),
        Ok(node) => {
            let live = probe::probe(node);
            let code = match live.state {
                State::Ok | State::Present => EXIT_OK,
                State::Unavailable | State::Unprobed => EXIT_UNAVAILABLE,
                State::Error | State::Missing => EXIT_RUNTIME,
            };
            if json {
                emit_and_exit(
                    envelope::ok(json!({ "live": serde_json::to_value(&live).unwrap_or(json!({})) })),
                    code,
                );
            }
            println!(
                "{} {} {}  {} {}",
                lamp(live.state),
                strong(&node.name),
                live.version.clone().unwrap_or_default(),
                live.state.label(),
                dim(&live.detail)
            );
            std::process::exit(code);
        }
    }
}

// ── run / open ──────────────────────────────────────────────────────────────
pub fn cmd_run(reg: &Registry, id: &str, args: &[String], json: bool) -> ! {
    match find_node(reg, id) {
        Err((e, fix)) => bad_args(&e, &fix, json),
        Ok(node) => {
            let Some(bin) = &node.bin else {
                let (e, fix) = (
                    format!("`{id}` has no binary registered ({})", node.kind.label()),
                    format!("try `concourse open {id}`, or add a `bin` to its registry entry"),
                );
                if json {
                    emit_and_exit(envelope::err(&e, &fix), EXIT_BAD_ARGS);
                }
                envelope::human_err_exit(&e, &fix, EXIT_BAD_ARGS);
            };
            use std::os::unix::process::CommandExt;
            let err = std::process::Command::new(util::expand_home(bin)).args(args).exec();
            // exec only returns on failure
            let (e, fix) = (
                format!("could not exec {bin}: {err}"),
                format!("check `concourse probe {id}` — the binary may be missing"),
            );
            if json {
                emit_and_exit(envelope::err(&e, &fix), EXIT_RUNTIME);
            }
            envelope::human_err_exit(&e, &fix, EXIT_RUNTIME);
        }
    }
}

pub fn open_node(node: &Node) -> Result<String, String> {
    match &node.open {
        None => Err(format!("`{}` has no open action declared", node.id)),
        Some(OpenSpec::Path { path }) => {
            let p = util::expand_home(path);
            util::xdg_open(&p)?;
            Ok(p.to_string_lossy().into_owned())
        }
        Some(OpenSpec::App { argv, cwd }) => {
            let cwd_p = cwd.as_ref().map(|c| util::expand_home(c));
            util::spawn_detached(argv, cwd_p.as_ref())?;
            Ok(argv.join(" "))
        }
    }
}

pub fn cmd_open(reg: &Registry, id: &str, json: bool) -> ! {
    match find_node(reg, id) {
        Err((e, fix)) => bad_args(&e, &fix, json),
        Ok(node) => match open_node(node) {
            Ok(what) => {
                if json {
                    emit_and_exit(envelope::ok(json!({ "opened": what })), EXIT_OK);
                }
                println!("opened {} {}", node.glyph, strong(&node.name));
                std::process::exit(EXIT_OK);
            }
            Err(e) => {
                let fix = format!("add an `open` action to `{id}` in ~/.config/concourse/registry.json");
                if json {
                    emit_and_exit(envelope::err(&e, &fix), EXIT_UNAVAILABLE);
                }
                envelope::human_err_exit(&e, &fix, EXIT_UNAVAILABLE);
            }
        },
    }
}

// ── doctor / skills / asks / cabinet ───────────────────────────────────────
pub fn cmd_doctor(reg: &Registry, json: bool) -> ! {
    let report = crate::doctor::run(reg);
    let code = if report.fail > 0 { EXIT_RUNTIME } else { EXIT_OK };
    if json {
        emit_and_exit(
            envelope::ok(serde_json::to_value(&report).unwrap_or(json!({}))),
            code,
        );
    }
    println!("{}", strong("concourse doctor — the estate, checked"));
    let mut last_subject = String::new();
    for fdg in &report.findings {
        if fdg.subject != last_subject {
            println!("{}", strong(&fdg.subject));
            last_subject = fdg.subject.clone();
        }
        let mark = match fdg.level {
            crate::doctor::Level::Pass => paint("38;5;114", "✓"),
            crate::doctor::Level::Warn => paint("38;5;214", "⚠"),
            crate::doctor::Level::Fail => paint("38;5;167", "✗"),
        };
        println!("  {mark} {:<12} {}", fdg.check, dim(&fdg.note));
    }
    println!(
        "{}",
        strong(&format!(
            "{} pass · {} warn · {} fail",
            report.pass, report.warn, report.fail
        ))
    );
    std::process::exit(code);
}

pub fn cmd_skills(reg: &Registry, json: bool) -> ! {
    let sk = crate::skills::report(&reg.skill_mirrors);
    if json {
        emit_and_exit(
            envelope::ok(serde_json::to_value(&sk).unwrap_or(json!({}))),
            EXIT_OK,
        );
    }
    println!(
        "{}  {} claude · {} hermes",
        strong("skills"),
        sk.claude.len(),
        sk.hermes.len()
    );
    println!("{}", strong("mirrors (sync only on Ben's ping)"));
    for m in &sk.mirrors {
        let mark = if m.state == "paired" {
            paint("38;5;114", "✓")
        } else {
            paint("38;5;214", "⚠")
        };
        println!("  {mark} {:<38} ↔ {:<28} {}", m.claude, m.hermes, dim(&m.state));
    }
    std::process::exit(EXIT_OK);
}

pub fn cmd_asks(reg: &Registry, rest: &[String], json: bool) -> ! {
    let _ = reg;
    if rest.first().map(String::as_str) == Some("add") {
        let mut by = "ben".to_string();
        let mut words = Vec::new();
        let mut it = rest[1..].iter();
        while let Some(w) = it.next() {
            if w == "--by" {
                if let Some(v) = it.next() {
                    by = v.clone();
                }
            } else {
                words.push(w.clone());
            }
        }
        let text = words.join(" ");
        if text.trim().is_empty() {
            bad_args(
                "asks add needs text",
                "concourse asks add \"the ask\" [--by ben|claude|nexus]",
                json,
            );
        }
        match crate::asks::add(&by, &text) {
            Ok(ask) => {
                if json {
                    emit_and_exit(
                        envelope::ok(json!({ "added": serde_json::to_value(&ask).unwrap_or(json!({})) })),
                        EXIT_OK,
                    );
                }
                println!("noted — {} ({})", ask.text, ask.by);
                std::process::exit(EXIT_OK);
            }
            Err(e) => runtime_err(&format!("could not append: {e}"), "check ~/.local/share is writable", json),
        }
    }
    match crate::asks::list() {
        Ok(asks) => {
            if json {
                emit_and_exit(
                    envelope::ok(json!({
                        "ledger": crate::asks::ledger_path().to_string_lossy(),
                        "asks": serde_json::to_value(&asks).unwrap_or(json!([])),
                    })),
                    EXIT_OK,
                );
            }
            if asks.is_empty() {
                println!("the ledger is empty — `concourse asks add \"…\"` starts it");
            }
            for a in asks {
                println!(" {} {:<7} {}", dim(&a.ts), format!("[{}]", a.by), a.text);
            }
            std::process::exit(EXIT_OK);
        }
        Err(e) => runtime_err(&format!("could not read ledger: {e}"), "check ~/.local/share/concourse", json),
    }
}

pub fn cmd_cabinet(full: bool, json: bool) -> ! {
    let path = util::cabinet_path();
    if !path.is_file() {
        let e = "~/Dev/ClaudeWorkspace/AGENTS.md is missing — the filing cabinet is the house's law";
        let fix = "restore it from git or ask Ben; see also `concourse doctor`";
        if json {
            emit_and_exit(envelope::err(e, fix), EXIT_UNAVAILABLE);
        }
        envelope::human_err_exit(e, fix, EXIT_UNAVAILABLE);
    }
    let bytes = std::fs::read(&path).unwrap_or_default();
    use sha2::{Digest, Sha256};
    let sha = format!("{:x}", Sha256::digest(&bytes));
    use std::os::unix::fs::PermissionsExt;
    let meta = std::fs::metadata(&path).ok();
    let readonly = meta
        .as_ref()
        .map(|m| m.permissions().mode() & 0o222 == 0)
        .unwrap_or(false);
    let mtime = meta
        .and_then(|m| m.modified().ok())
        .map(|t| chrono::DateTime::<chrono::Local>::from(t).to_rfc3339_opts(chrono::SecondsFormat::Secs, false))
        .unwrap_or_default();
    if json {
        let mut payload = json!({
            "path": path.to_string_lossy(),
            "readonly": readonly,
            "sha256": sha,
            "mtime": mtime,
            "bytes": bytes.len(),
        });
        if full {
            payload["text"] = json!(String::from_utf8_lossy(&bytes));
        }
        emit_and_exit(envelope::ok(payload), EXIT_OK);
    }
    println!("{}  {}", strong("🗄 the filing cabinet"), dim(&path.to_string_lossy()));
    println!(
        "  {} · sha {} · tended {}",
        if readonly { "sealed (Ben's hand only)" } else { "WRITABLE — reseal with chmod 444" },
        &sha[..12],
        mtime
    );
    if full {
        println!("{}", String::from_utf8_lossy(&bytes));
    } else {
        println!("  read it: concourse open cabinet   (or --full here)");
    }
    std::process::exit(EXIT_OK);
}

// ── attention (the estate's doc-link index) ────────────────────────────────
pub fn cmd_attention(rest: &[String], json: bool) -> ! {
    match rest.first().map(String::as_str) {
        Some("scan") => match crate::attention::scan() {
            Ok(st) => {
                if json {
                    emit_and_exit(envelope::ok(serde_json::to_value(&st).unwrap_or(json!({}))), EXIT_OK);
                }
                println!(
                    "{}  {} docs · {} link pairs · {} refs · {}ms",
                    strong("attention index rebuilt"),
                    st.docs,
                    st.link_pairs,
                    st.refs_total,
                    st.elapsed_ms
                );
                println!("  {}", dim(&st.db));
                std::process::exit(EXIT_OK);
            }
            Err(e) => runtime_err(&e, "check ~/.local/share is writable", json),
        },
        Some("edges") => match crate::attention::load_graph() {
            Ok(g) => {
                let subdirs = rest.iter().any(|a| a == "--subdirs");
                let fg = crate::attention::folder_graph(&g, subdirs);
                if json {
                    emit_and_exit(
                        envelope::ok(json!({
                            "scanned_at": g.scanned_at,
                            "folders": serde_json::to_value(&fg.folders).unwrap_or(json!([])),
                            "edges": fg.edges.iter().map(|e| json!({
                                "a": fg.folders[e.a].key, "b": fg.folders[e.b].key,
                                "a_to_b": e.n_ab, "b_to_a": e.n_ba,
                                "mutual": e.n_ab > 0 && e.n_ba > 0,
                            })).collect::<Vec<_>>(),
                        })),
                        EXIT_OK,
                    );
                }
                println!("{}  (scanned {})", strong("attention edges — folder level"), dim(&g.scanned_at));
                for e in fg.edges.iter().take(40) {
                    let (a, b) = (&fg.folders[e.a], &fg.folders[e.b]);
                    let arrow = if e.n_ab > 0 && e.n_ba > 0 { "<-->" } else if e.n_ab > 0 { "-->" } else { "<--" };
                    println!(
                        "  {:<28} {} {:<28} {}",
                        a.key,
                        arrow,
                        b.key,
                        dim(&format!("{}·{}", e.n_ab, e.n_ba))
                    );
                }
                std::process::exit(EXIT_OK);
            }
            Err(e) => runtime_err(&e, "run `concourse attention scan` first", json),
        },
        None => match crate::attention::load_graph() {
            Ok(g) => {
                if json {
                    emit_and_exit(
                        envelope::ok(json!({
                            "scanned_at": g.scanned_at,
                            "docs": g.docs.len(),
                            "links": g.links.len(),
                            "db": crate::attention::db_path().to_string_lossy(),
                            "workflow": "~/Dev/ClaudeWorkspace/concourse/docs/ATTENTION-WORKFLOW.md",
                        })),
                        EXIT_OK,
                    );
                }
                println!(
                    "{}  {} docs · {} link pairs · scanned {}",
                    strong("the attention index"),
                    g.docs.len(),
                    g.links.len(),
                    dim(&g.scanned_at)
                );
                println!("  db {}", dim(&crate::attention::db_path().to_string_lossy()));
                println!("  refresh: concourse attention scan · workflow: docs/ATTENTION-WORKFLOW.md");
                std::process::exit(EXIT_OK);
            }
            Err(e) => {
                let fix = "run `concourse attention scan` to build it";
                if json {
                    emit_and_exit(envelope::err(&e, fix), EXIT_UNAVAILABLE);
                }
                envelope::human_err_exit(&e, fix, EXIT_UNAVAILABLE);
            }
        },
        Some(other) => bad_args(
            &format!("unknown attention subverb `{other}`"),
            "concourse attention [scan|edges] [--json]",
            json,
        ),
    }
}

// ── systemd services + the job ledger ──────────────────────────────────────
pub fn cmd_services(rest: &[String], json: bool) -> ! {
    let want_user = rest.iter().any(|a| a == "--user");
    let want_system = rest.iter().any(|a| a == "--system");
    let scopes: Vec<bool> = match (want_user, want_system) {
        (true, false) => vec![true],
        (false, true) => vec![false],
        _ => vec![false, true],
    };
    let mut units = Vec::new();
    let mut errors = Vec::new();
    for user in scopes {
        match crate::services::list_units(user) {
            Ok(mut u) => units.append(&mut u),
            Err(e) => errors.push(e),
        }
    }
    if units.is_empty() && !errors.is_empty() {
        runtime_err(&errors.join("; "), "is systemd running? `systemctl --version`", json);
    }
    let jobs = crate::services::list_jobs();
    if json {
        emit_and_exit(
            envelope::ok(json!({
                "units": units.iter().map(|u| {
                    let last = crate::services::latest_for(&jobs, &u.name, &u.scope);
                    json!({
                        "name": u.name, "scope": u.scope, "description": u.description,
                        "active": u.active, "sub": u.sub, "enabled": u.enabled,
                        "last_job": last.map(|j| crate::services::job_json(&j)),
                    })
                }).collect::<Vec<_>>(),
                "warnings": errors,
            })),
            EXIT_OK,
        );
    }
    println!("{}", strong("systemd through the hall — services & timers"));
    for u in &units {
        let lamp = match (u.active.as_str(), u.sub.as_str()) {
            ("failed", _) | (_, "failed") => paint("38;5;167", "✗"),
            ("active", _) => paint("38;5;114", "●"),
            _ => paint("38;5;245", "·"),
        };
        println!(
            " {} {:<44} {:<9} {:<9} {}",
            lamp,
            u.name.chars().take(44).collect::<String>(),
            u.enabled.chars().take(9).collect::<String>(),
            u.active,
            dim(&format!("[{}]", u.scope)),
        );
    }
    for e in &errors {
        println!(" {} {}", paint("38;5;214", "⚠"), dim(e));
    }
    std::process::exit(EXIT_OK);
}

pub fn cmd_jobs(json: bool) -> ! {
    let jobs = crate::services::list_jobs();
    if json {
        emit_and_exit(
            envelope::ok(json!({
                "dir": crate::services::jobs_dir().to_string_lossy(),
                "jobs": jobs.iter().map(crate::services::job_json).collect::<Vec<_>>(),
            })),
            EXIT_OK,
        );
    }
    if jobs.is_empty() {
        println!("no jobs yet — the services panel (or `concourse job dispatch …`) starts one");
    }
    for j in &jobs {
        let mark = match j.status.as_str() {
            "ok" => paint("38;5;114", "●"),
            "fail" => paint("38;5;167", "✗"),
            _ => paint("38;5;214", "⟳"),
        };
        println!(
            " {} {} {:<9} {} {} ({}) {}",
            mark,
            dim(&j.ts.chars().take(16).collect::<String>()),
            j.status,
            j.action,
            strong(&j.unit),
            j.harness,
            dim(&j.note.chars().take(48).collect::<String>()),
        );
    }
    std::process::exit(EXIT_OK);
}

pub fn cmd_job(rest: &[String], json: bool) -> ! {
    match rest.first().map(String::as_str) {
        Some("report") => {
            let (Some(id), Some(status)) = (rest.get(1), rest.get(2)) else {
                bad_args(
                    "job report needs an id and a status",
                    "concourse job report <id> ok|fail [--note \"…\"]",
                    json,
                );
            };
            if status != "ok" && status != "fail" {
                bad_args(
                    &format!("status must be ok or fail, got `{status}`"),
                    "concourse job report <id> ok|fail [--note \"…\"]",
                    json,
                );
            }
            let mut note = String::new();
            let mut it = rest[3..].iter();
            while let Some(w) = it.next() {
                if w == "--note" {
                    if let Some(v) = it.next() {
                        note = v.clone();
                    }
                }
            }
            match crate::services::report(id, status, &note) {
                Ok(job) => {
                    if json {
                        emit_and_exit(envelope::ok(json!({ "job": crate::services::job_json(&job) })), EXIT_OK);
                    }
                    println!("job {} → {} ({})", job.id, job.status, job.unit);
                    std::process::exit(EXIT_OK);
                }
                Err(e) => runtime_err(&e, "concourse jobs --json lists real ids", json),
            }
        }
        Some("dispatch") => {
            let (Some(unit), Some(action)) = (rest.get(1), rest.get(2)) else {
                bad_args(
                    "job dispatch needs a unit and an action",
                    "concourse job dispatch <unit> enable|disable [--scope system|user] [--harness claude|hermes|opencode]",
                    json,
                );
            };
            let mut scope = "system".to_string();
            let mut harness = "claude".to_string();
            let mut it = rest[3..].iter();
            while let Some(w) = it.next() {
                match w.as_str() {
                    "--scope" => {
                        if let Some(v) = it.next() {
                            scope = v.clone();
                        }
                    }
                    "--harness" => {
                        if let Some(v) = it.next() {
                            harness = v.clone();
                        }
                    }
                    _ => {}
                }
            }
            match crate::services::dispatch(unit, &scope, action, &harness) {
                Ok(job) => {
                    if json {
                        emit_and_exit(envelope::ok(json!({ "job": crate::services::job_json(&job) })), EXIT_OK);
                    }
                    println!(
                        "dispatched {} to {} {} ({}) — log {}",
                        job.harness, job.action, job.unit, job.scope, job.log
                    );
                    std::process::exit(EXIT_OK);
                }
                Err(e) => runtime_err(&e, "check the harness is installed and the unit name is exact", json),
            }
        }
        _ => bad_args(
            "job needs a subverb",
            "concourse job report <id> ok|fail [--note …] · concourse job dispatch <unit> enable|disable [--scope …] [--harness …]",
            json,
        ),
    }
}

// ── error helpers ───────────────────────────────────────────────────────────
pub fn bad_args(error: &str, fix: &str, json: bool) -> ! {
    if json {
        emit_and_exit(envelope::err(error, fix), EXIT_BAD_ARGS);
    }
    envelope::human_err_exit(error, fix, EXIT_BAD_ARGS);
}

fn runtime_err(error: &str, fix: &str, json: bool) -> ! {
    if json {
        emit_and_exit(envelope::err(error, fix), EXIT_RUNTIME);
    }
    envelope::human_err_exit(error, fix, EXIT_RUNTIME);
}
