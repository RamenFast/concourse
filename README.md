# 🏛 Concourse — the station's shared hall

*One hub where Ben and every agent system (Claude, Nexus/Hermes, whatever comes next) see the
same estate and reach every tool the same way. A first-party GUI for the humans, the same
verbs as JSON for the agents — both renderers over one core, so neither can drift.*

Built 2026-07-07 by Claude Fable 5 with Ben. Rust + egui + SQLite. GPLv3.
Part of the station: [`NexusFormStationWork`](../NexusFormStationWork/) is the bench;
[`AGENT-CLI-STANDARD.md`](../AGENT-CLI-STANDARD.md) is the law it enforces;
[`~/Dev/ClaudeWorkspace/AGENTS.md`](../AGENTS.md) is the filing cabinet it shows.

## The hall (GUI)

`concourse` (or the **Concourse** menu entry) opens one window with two renders —
**🏛 hall** (departures + bays) and **🧭 estate** (the attention map) — switched in the top bar:

| room | what |
|---|---|
| **DEPARTURES** | live board of every tool & service — probed concurrently, semantic lamps, click a row for its inspector |
| **the bays** | every project/app as a card, grouped by district (Town Hall · Instruments · Workshop · Library · Arcade) with open / folder / signpost buttons |
| **🧭 estate map** | the bays as a top-down folder structure with attention flow: arrows where `.md`/`.html` docs reference each other (orange = Claude, pink = Nexus, grey = opencode/misc; `-->` one-way weaker, `<-->` mutual stronger, brightness scales with connection count; archives never indexed). Collapsible control panel: reindex, view options, hide folders (ghosts optional), agent legend, connections list. Every folder opens in a terminal — the manual agent entry ([workflow](docs/ATTENTION-WORKFLOW.md)) |
| **⚙ services** | systemd services & timers, both scopes — enable/disable **dispatches an agent** (claude / hermes / opencode, default remembered) that does the work and reports back; green = managed as asked, red = could not continue (agent log one click away) |
| **🗄 cabinet** | the governance doc, rendered read-only with its seal, hash, and tended date |
| **🎓 skills** | the Claude/Hermes skill estate and mirror drift — category folders on the left (📁 opens, name filters), vertical-resize fixed |
| **📒 asks** | the asks ledger — every unique ask, remembered |
| **🩺 doctor** | the standard, enforced: probes, envelopes, branch law, governance wiring |

The ⟳ REFRESH stone re-probes everything; Esc dismisses the newest window first;
popouts persist until dismissed. Blossom Dark by default; the theme stone cycles
Blossom / Light / Dark.

## The verbs (CLI — same core, both audiences)

```bash
concourse status            # the whole board, one shot (pretty on a TTY)
concourse status --json     # the same board as one envelope (agents)
concourse nodes             # the registry, no probing
concourse node phosphor     # one node: entry + live probe
concourse probe wisp        # exit code mirrors its state (0/2/4)
concourse run surveyor status   # exec any registered binary — no path memorized
concourse open station      # open a node for a human (app/page/folder)
concourse doctor            # audit the estate against the standard
concourse skills            # skill mirrors + drift
concourse asks add "…" --by ben
concourse cabinet --full    # the governance doc + its seal
concourse attention scan    # rebuild the estate's doc-link index (SQLite, ~5 s)
concourse attention edges   # folder-level attention flow — the map's arrows
concourse services --user   # systemd units with enablement + last dispatch job
concourse job dispatch foo.service enable --scope user --harness claude
concourse job report j…-… ok --note "verified"   # how a dispatched agent reports back
concourse jobs              # the dispatch ledger
concourse schema            # the full contract, machine-readable
```

Envelope on every one-shot (`status/tool/version/ts`, errors carry `fix`), exit codes
`0/2/3/4`, isatty auto-switch, `--json` forces. The hub obeys the standard it audits.

## The registry (config is data)

`~/.config/concourse/registry.json` — seeded from [`assets/registry.json`](assets/registry.json)
on first run. A node is `{id, kind: tool|service|place|page, district, probe, open, signpost,
skills, verbs}`. Add a node there and the GUI grows its card and board row on the next refresh;
no code changes. `CONCOURSE_REGISTRY=…` overrides (tests).

## Build & install

```bash
make install     # cargo build --release + binary → ~/.local/bin + .desktop + hicolor icons
```

(Already done on Ben's machine — this is for rebuilds. `Cargo.toml` carries
`generate-rpm` metadata for a packaged release day.)

## Honest ledger

- Probes run concurrently with per-node timeouts; a dead node degrades to its honest state
  (`missing`/`unavailable`/`error`), never breaks the board.
- The attention index is deterministic heuristics (markdown links, wikilinks, href/src,
  bare `~/` paths) — counts are honest approximations of attention, not ground truth;
  the scanner's roots and rules live in `src/attention.rs` and the agent workflow in
  [`docs/ATTENTION-WORKFLOW.md`](docs/ATTENTION-WORKFLOW.md).
- Service dispatch was verified end-to-end on day one: a self-test dispatch on a
  nonexistent unit had the claude agent investigate, refuse to fabricate a unit file,
  and report `fail` with the fix named — exactly the designed behavior.
- `doctor` found real things on day one: phosphor's 13 extra branches, surveyor's envelope
  drift (rulings R1/R2), wisp's missing binary. That's the point.
- Known soft spots live in [`docs/SERIOUS-TODOS.md`](docs/SERIOUS-TODOS.md) — the
  metacognitive ledger. Every founding ask: [`docs/ASKS.md`](docs/ASKS.md).
