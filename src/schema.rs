// SPDX-License-Identifier: GPL-3.0-or-later
//! `concourse schema` — the whole contract, machine-readable, envelope-carrying
//! (ruling R5: discovery verbs are one-shots too). An agent with zero prior
//! training goes from this object to a correct call.

use serde_json::{json, Value};

pub fn schema() -> Value {
    crate::envelope::ok(json!({
        "convention": "~/Dev/ClaudeWorkspace/AGENT-CLI-STANDARD.md (normative core: NexusFormStationWork/CONVENTION.md)",
        "exit_codes": { "0": "ok", "2": "unavailable", "3": "bad arguments", "4": "runtime failure" },
        "output_mode": "pretty prose on a TTY; JSON when piped; --json forces JSON",
        "envelope": {
            "every_one_shot_carries": ["status", "tool", "version", "ts"],
            "on_error_adds": ["error", "fix"],
            "additionalProperties": false
        },
        "verbs": {
            "status": {
                "what": "probe every registered node concurrently; one board, one object",
                "args": [],
                "output": {
                    "nodes": "[{id, state: ok|unavailable|error|missing|present|unprobed, version?, detail, latency_ms}]",
                    "governance": "{cabinet_path, present, readonly, sha256_12}",
                    "disk_free_bytes": "number",
                    "additionalProperties": false
                }
            },
            "nodes": { "what": "the registry itself (no probing)", "args": [], "output": { "districts": "[string]", "nodes": "[registry entries]", "additionalProperties": false } },
            "node": { "what": "one node: registry entry + live probe", "args": ["<id>"], "output": { "node": "registry entry", "live": "probe result", "additionalProperties": false } },
            "probe": { "what": "live-probe one node", "args": ["<id>"], "output": { "live": "{id, state, version?, detail, latency_ms, raw?}", "additionalProperties": false } },
            "run": { "what": "exec a node's binary with your args (resolves the path so you don't hold wiring in your head); exit code passes through", "args": ["<id>", "[args…]"], "output": "the tool's own output, untouched" },
            "open": { "what": "open a node for a human: launch its app or xdg-open its page/folder", "args": ["<id>"], "output": { "opened": "string", "additionalProperties": false } },
            "doctor": { "what": "audit the estate against the standard: probes, envelope fields, branch discipline, governance wiring, skill mirrors", "args": [], "output": { "pass": "n", "warn": "n", "fail": "n", "findings": "[{subject, check, level, note}]", "additionalProperties": false } },
            "skills": { "what": "scan ~/.claude/skills and ~/.hermes/skills; report mirror pairs and drift (report only — syncing waits for Ben's ping)", "args": [], "output": { "claude": "[{name, description, path}]", "hermes": "[…]", "mirrors": "[{claude, hermes, state}]", "additionalProperties": false } },
            "asks": { "what": "the asks ledger (append-only NDJSON at ~/.local/share/concourse/asks.ndjson)", "args": ["[add <text> [--by NAME]]"], "output": { "asks": "[{ts, by, text}]", "additionalProperties": false } },
            "cabinet": { "what": "the governance doc's metadata (path, mode, sha256, mtime); --full includes the text", "args": ["[--full]"], "output": { "path": "string", "readonly": "bool", "sha256": "string", "mtime": "string", "bytes": "n", "text?": "with --full", "additionalProperties": false } },
            "schema": { "what": "this object", "args": [], "output": "you are reading it" },
            "gui": { "what": "launch the GUI hall (bare `concourse` does the same)", "args": [], "output": "a window" }
        },
        "registry": {
            "path": "~/.config/concourse/registry.json (env CONCOURSE_REGISTRY overrides)",
            "seeded_from": "concourse repo assets/registry.json on first run",
            "node_kinds": ["tool", "service", "place", "page"],
            "probe_kinds": ["cmd", "http (loopback only)", "file", "dir"]
        }
    }))
}
