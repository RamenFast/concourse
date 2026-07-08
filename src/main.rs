// SPDX-License-Identifier: GPL-3.0-or-later
//! concourse — the station's shared hall.
//! One hub where Ben and every agent see and reach the whole estate:
//! a GUI for the humans, the same verbs as JSON for the agents,
//! both renderers over one core. Bare `concourse` opens the hall.

mod asks;
mod attention;
mod doctor;
mod envelope;
mod gui;
mod probe;
mod registry;
mod schema;
mod services;
mod skills;
mod util;
mod verbs;

const HELP: &str = "\
concourse 0.1.0 — the station's shared hall (human + agent hub)

usage:
  concourse                     open the hall (GUI)
  concourse status [--json]     probe every node; the whole board, one shot
  concourse nodes  [--json]     the registry (no probing)
  concourse node ID [--json]    one node: entry + live probe
  concourse probe ID [--json]   live-probe one node (exit mirrors its state)
  concourse run ID [ARGS…]      exec a node's binary (path resolved for you)
  concourse open ID             open a node (app / page / folder)
  concourse doctor [--json]     audit the estate against the standard
  concourse skills [--json]     claude/hermes skill estate + mirror drift
  concourse asks [add TEXT [--by NAME]] [--json]   the asks ledger
  concourse cabinet [--full] [--json]              the governance doc
  concourse attention [scan|edges] [--json]        the estate's doc-link index
  concourse services [--user|--system] [--json]    systemd services & timers, both scopes
  concourse jobs [--json]       agent-dispatch job ledger
  concourse job report ID ok|fail [--note TEXT]    how a dispatched agent reports back
  concourse job dispatch UNIT enable|disable [--scope system|user] [--harness NAME]
  concourse schema              the machine-readable contract
  concourse gui                 open the hall explicitly

exit codes: 0 ok · 2 unavailable · 3 bad arguments · 4 runtime
the law: ~/Dev/ClaudeWorkspace/AGENTS.md · the standard: ~/Dev/ClaudeWorkspace/AGENT-CLI-STANDARD.md";

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let json_flag = args.iter().any(|a| a == "--json");
    args.retain(|a| a != "--json");
    let json = envelope::json_mode(json_flag);

    // bare invocation: the hall itself (a GUI app first, like phosphor)
    if args.is_empty() {
        launch_gui(json_flag);
    }

    let verb = args[0].clone();
    let rest = &args[1..];

    // registry-independent verbs first
    match verb.as_str() {
        "-h" | "--help" | "help" => {
            println!("{HELP}");
            std::process::exit(envelope::EXIT_OK);
        }
        "-V" | "--version" | "version" => {
            if json {
                envelope::emit_and_exit(envelope::ok(serde_json::json!({})), envelope::EXIT_OK);
            }
            println!("concourse {}", envelope::VERSION);
            std::process::exit(envelope::EXIT_OK);
        }
        "schema" => envelope::emit_and_exit(schema::schema(), envelope::EXIT_OK),
        "cabinet" => {
            let full = rest.iter().any(|a| a == "--full");
            verbs::cmd_cabinet(full, json);
        }
        "gui" => launch_gui(json_flag),
        _ => {}
    }

    let reg = match registry::load() {
        Ok(r) => r,
        Err((e, fix)) => {
            if json {
                envelope::emit_and_exit(envelope::err(&e, &fix), envelope::EXIT_RUNTIME);
            }
            envelope::human_err_exit(&e, &fix, envelope::EXIT_RUNTIME);
        }
    };

    match verb.as_str() {
        "status" => verbs::cmd_status(&reg, json),
        "nodes" => verbs::cmd_nodes(&reg, json),
        "node" => match rest.first() {
            Some(id) => verbs::cmd_node(&reg, id, json),
            None => verbs::bad_args("node needs an id", "concourse node phosphor — or `concourse nodes`", json),
        },
        "probe" => match rest.first() {
            Some(id) => verbs::cmd_probe(&reg, id, json),
            None => verbs::bad_args("probe needs an id", "concourse probe phosphor — or `concourse nodes`", json),
        },
        "run" => match rest.first() {
            Some(id) => verbs::cmd_run(&reg, id, &rest[1..], json),
            None => verbs::bad_args("run needs an id", "concourse run surveyor status", json),
        },
        "open" => match rest.first() {
            Some(id) => verbs::cmd_open(&reg, id, json),
            None => verbs::bad_args("open needs an id", "concourse open station", json),
        },
        "doctor" => verbs::cmd_doctor(&reg, json),
        "skills" => verbs::cmd_skills(&reg, json),
        "asks" => verbs::cmd_asks(&reg, rest, json),
        "attention" => verbs::cmd_attention(rest, json),
        "services" => verbs::cmd_services(rest, json),
        "jobs" => verbs::cmd_jobs(json),
        "job" => verbs::cmd_job(rest, json),
        "map" => verbs::cmd_open(&reg, "station", json),
        other => verbs::bad_args(
            &format!("unknown verb `{other}`"),
            "concourse --help names every verb; `concourse schema` is the full contract",
            json,
        ),
    }
}

fn launch_gui(json_flag: bool) -> ! {
    let has_display =
        std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some();
    if !has_display {
        let e = "no display — the hall is a window";
        let fix = "run inside a session (or Xvfb for agents); the CLI works everywhere: concourse status";
        if json_flag || !util::stdout_is_tty() {
            envelope::emit_and_exit(envelope::err(e, fix), envelope::EXIT_UNAVAILABLE);
        }
        envelope::human_err_exit(e, fix, envelope::EXIT_UNAVAILABLE);
    }
    match gui::run() {
        Ok(()) => std::process::exit(envelope::EXIT_OK),
        Err(e) => {
            envelope::human_err_exit(
                &format!("the hall could not open: {e}"),
                "check GPU/GL drivers; `concourse status` still works",
                envelope::EXIT_RUNTIME,
            );
        }
    }
}
