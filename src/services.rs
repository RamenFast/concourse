// SPDX-License-Identifier: GPL-3.0-or-later
//! Systemd through the hall: see every service/timer (system + user scope),
//! and manage them the estate's way — an enable/disable click doesn't run
//! systemctl itself, it dispatches an agent (claude / hermes / opencode,
//! Ben's pick, default remembered) that does the work with judgement and
//! reports back over the standard CLI:
//!     concourse job report <id> ok|fail --note "…"
//! Green = systemd managed the way Ben wants · red = could not continue,
//! the agent's log says why (one click away).

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;

// ── units ───────────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct Unit {
    pub name: String,
    pub scope: String, // "system" | "user"
    pub description: String,
    pub active: String,  // active | inactive | failed | …
    pub sub: String,     // running | dead | exited | …
    pub enabled: String, // enabled | disabled | static | masked | … | "?"
}

#[derive(Deserialize)]
struct LuRow {
    unit: String,
    #[serde(default)]
    load: String,
    #[serde(default)]
    active: String,
    #[serde(default)]
    sub: String,
    #[serde(default)]
    description: String,
}

#[derive(Deserialize)]
struct UfRow {
    unit_file: String,
    #[serde(default)]
    state: String,
}

pub fn list_units(user: bool) -> Result<Vec<Unit>, String> {
    let scope = if user { "user" } else { "system" };
    let base = |sub: &str| -> Vec<String> {
        let mut v = vec!["systemctl".to_string()];
        if user {
            v.push("--user".into());
        }
        v.extend([sub.into(), "--type=service,timer".into(), "--all".into(), "--no-pager".into(), "--output=json".into()]);
        v
    };
    let lu = crate::util::run_with_timeout(&base("list-units"), 8000, None);
    if lu.exit != Some(0) {
        return Err(format!(
            "systemctl {} list-units failed: {}",
            if user { "--user" } else { "--system" },
            lu.stderr.lines().next().unwrap_or("(no output)")
        ));
    }
    let rows: Vec<LuRow> = serde_json::from_str(&lu.stdout).map_err(|e| format!("systemctl json: {e}"))?;

    // enablement lives in list-unit-files — join by name
    let mut uf_args = base("list-unit-files");
    uf_args.retain(|a| a != "--all");
    let uf = crate::util::run_with_timeout(&uf_args, 8000, None);
    let states: std::collections::HashMap<String, String> = serde_json::from_str::<Vec<UfRow>>(&uf.stdout)
        .unwrap_or_default()
        .into_iter()
        .map(|r| (r.unit_file, r.state))
        .collect();

    let mut units: Vec<Unit> = rows
        .into_iter()
        .filter(|r| r.load != "not-found") // referenced-but-absent ghosts aren't Ben's units
        .map(|r| {
            let enabled = states.get(&r.unit).cloned().unwrap_or_else(|| "?".into());
            Unit {
                name: r.unit,
                scope: scope.into(),
                description: r.description,
                active: r.active,
                sub: r.sub,
                enabled,
            }
        })
        .collect();
    units.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(units)
}

// ── the job ledger ──────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
pub struct Job {
    pub id: String,
    pub ts: String,
    pub unit: String,
    pub scope: String,   // system | user
    pub action: String,  // enable | disable
    pub harness: String, // claude | hermes | opencode
    pub status: String,  // dispatched | ok | fail
    #[serde(default)]
    pub note: String,
    pub log: String, // absolute path to the agent transcript
    #[serde(default)]
    pub reported_ts: String,
}

pub fn jobs_dir() -> PathBuf {
    crate::util::data_dir().join("jobs")
}

pub fn list_jobs() -> Vec<Job> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(jobs_dir()) else { return out };
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) == Some("json") {
            if let Ok(text) = std::fs::read_to_string(&p) {
                if let Ok(job) = serde_json::from_str::<Job>(&text) {
                    out.push(job);
                }
            }
        }
    }
    out.sort_by(|a, b| a.ts.cmp(&b.ts));
    out
}

/// The newest job touching a unit (any scope match by name+scope).
pub fn latest_for(jobs: &[Job], unit: &str, scope: &str) -> Option<Job> {
    jobs.iter().rev().find(|j| j.unit == unit && j.scope == scope).cloned()
}

fn write_job(job: &Job) -> Result<(), String> {
    let dir = jobs_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.json", job.id));
    std::fs::write(&path, serde_json::to_string_pretty(job).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

pub fn report(id: &str, status: &str, note: &str) -> Result<Job, String> {
    let path = jobs_dir().join(format!("{id}.json"));
    let text = std::fs::read_to_string(&path)
        .map_err(|_| format!("no job `{id}` — `concourse jobs --json` lists real ids"))?;
    let mut job: Job = serde_json::from_str(&text).map_err(|e| format!("job file unreadable: {e}"))?;
    job.status = status.into();
    job.note = note.into();
    job.reported_ts = crate::util::now_ts();
    write_job(&job)?;
    Ok(job)
}

// ── dispatch ────────────────────────────────────────────────────────────────

pub const HARNESSES: [&str; 3] = ["claude", "hermes", "opencode"];

fn job_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!(
        "j{}-{:04}",
        chrono::Local::now().format("%Y%m%d-%H%M%S"),
        nanos % 10000
    )
}

fn prompt_for(job: &Job) -> String {
    let sysctl = if job.scope == "user" {
        format!("systemctl --user {} {}", job.action, job.unit)
    } else {
        format!("sudoplz sudo systemctl {} {}", job.action, job.unit)
    };
    let verify = if job.scope == "user" {
        format!("systemctl --user is-enabled {}", job.unit)
    } else {
        format!("systemctl is-enabled {}", job.unit)
    };
    format!(
        "You are dispatched by Concourse (job {id}) on Ben's machine. \
Task: make sure the systemd unit `{unit}` ({scope} scope) is {action}d. \
The straightforward path is `{sysctl}` — but you have judgement: read the unit's state first \
(is-enabled / status), and if something is off (masked, missing, a dependency, an alias), \
handle it sensibly or stop rather than force it. Never use bare sudo — on this machine \
system-scope commands go through `sudoplz sudo …`. Verify the end state with `{verify}`. \
When you are done you MUST report back over the standard CLI (this flips the Concourse UI \
green or red):\n\
  concourse job report {id} ok --note \"what you verified\"\n\
  concourse job report {id} fail --note \"why it could not continue\"\n\
Report fail honestly if the state is not what was asked. Your transcript is at {log}.",
        id = job.id,
        unit = job.unit,
        scope = job.scope,
        action = job.action,
        sysctl = sysctl,
        verify = verify,
        log = job.log,
    )
}

/// argv for each harness's non-interactive one-shot mode.
fn harness_argv(harness: &str, prompt: &str) -> Result<Vec<String>, String> {
    match harness {
        "claude" => Ok(vec![
            "claude".into(),
            "--dangerously-skip-permissions".into(),
            "-p".into(),
            prompt.into(),
        ]),
        "hermes" => Ok(vec!["hermes".into(), "chat".into(), "-q".into(), prompt.into()]),
        "opencode" => Ok(vec!["opencode".into(), "run".into(), prompt.into()]),
        other => Err(format!("unknown harness `{other}` — one of claude, hermes, opencode")),
    }
}

pub fn dispatch(unit: &str, scope: &str, action: &str, harness: &str) -> Result<Job, String> {
    if !matches!(action, "enable" | "disable") {
        return Err(format!("action must be enable or disable, got `{action}`"));
    }
    if !matches!(scope, "system" | "user") {
        return Err(format!("scope must be system or user, got `{scope}`"));
    }
    if !HARNESSES.contains(&harness) {
        return Err(format!("unknown harness `{harness}` — one of claude, hermes, opencode"));
    }
    if crate::util::on_path(harness).is_none() {
        return Err(format!("`{harness}` is not on PATH — pick another harness"));
    }
    let id = job_id();
    let dir = jobs_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let log = dir.join(format!("{id}.log"));
    let mut job = Job {
        id: id.clone(),
        ts: crate::util::now_ts(),
        unit: unit.into(),
        scope: scope.into(),
        action: action.into(),
        harness: harness.into(),
        status: "dispatched".into(),
        note: String::new(),
        log: log.to_string_lossy().into_owned(),
        reported_ts: String::new(),
    };
    let prompt = prompt_for(&job);
    let argv = harness_argv(harness, &prompt)?;

    // spawn detached, transcript captured — the header line names the dispatch
    let logfile = std::fs::File::create(&log).map_err(|e| e.to_string())?;
    use std::io::Write;
    let mut lf = &logfile;
    let _ = writeln!(
        lf,
        "── concourse job {id} · {} {} ({scope}) · harness {harness} · {} ──",
        action,
        unit,
        job.ts
    );
    let err_file = logfile.try_clone().map_err(|e| e.to_string())?;
    std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(logfile))
        .stderr(std::process::Stdio::from(err_file))
        .current_dir(crate::util::home())
        .spawn()
        .map_err(|e| {
            job.status = "fail".into();
            job.note = format!("could not spawn {harness}: {e}");
            let _ = write_job(&job);
            format!("could not spawn {harness}: {e}")
        })?;
    write_job(&job)?;
    Ok(job)
}

pub fn job_json(j: &Job) -> serde_json::Value {
    json!({
        "id": j.id, "ts": j.ts, "unit": j.unit, "scope": j.scope,
        "action": j.action, "harness": j.harness, "status": j.status,
        "note": j.note, "log": j.log, "reported_ts": j.reported_ts,
    })
}
