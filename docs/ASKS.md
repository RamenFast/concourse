# ASKS — every unique ask behind Concourse

*Ben's rule: keep track of every unique ask when building a program — it's how bugs get
rediscovered and intent survives sessions. Sources: the founding braindump (2026-07-07),
the station's STARTING-INTENTIONS (2026-07-04), and rules already in force via skills.
Each ask names where it landed.*

| # | ask (Ben's words, condensed) | where it landed |
|---|---|---|
| 1 | Combine the station project's intentions with the hub braindump | OUTCOMES.md (station repo) |
| 2 | Extract atomic end-goal expected outcomes | OUTCOMES.md — every outcome atomic + checkable |
| 3 | One interface where Ben, Nexus, and all agent systems share a human+AI readable space | this app: GUI + CLI over one core |
| 4 | Make sure other applications integrate | registry (30 nodes) · `open`/`run`/`probe` · AGENT-CLI-STANDARD audit |
| 5 | A Hermes skill for the hub | `~/.hermes/skills/station/concourse/SKILL.md` |
| 6 | Give it a good name | **Concourse** — the station's shared hall, where every line converges |
| 7 | Get creative, with polish | departures board, carved stones, districts, gentle empty states, Esc cascade |
| 8 | GUI app first party; CLI for humans; first-party CLI for agents | `concourse` (GUI) · pretty TTY verbs · `--json` everywhere |
| 9 | Consistent agent-native JSON CLI baked into every custom app; search for an existing standard; if lacking, a detailed .MD at workspace top level | found the Station Convention; promoted + audited in `~/Dev/ClaudeWorkspace/AGENT-CLI-STANDARD.md` |
| 10 | Use Phosphor as the good example; poke holes where found | standard §4: two real holes (schema envelope, bench outputs) + what it gets right |
| 11 | Claude manages GitHub end-to-end; only "push to master?" is asked; ≤1 branch per repo | `~/AGENTS.md` §4 · `concourse doctor` branch check (it already caught phosphor ×13) |
| 12 | Prefer desktop folders/workspaces (Tailscale, PC always on) | `~/AGENTS.md` §4 |
| 13 | A GUI window with buttons/links to every project/custom app | the bays: district cards with open/folder/signpost |
| 14 | Skill files connect agents to each app; Claude/Hermes stay separate; sync only on Ben's manual ping; progressive disclosure | `~/AGENTS.md` §2 · `concourse skills` mirror report (report-only) |
| 15 | A human-maintained filing cabinet: governance doc, no agent edits without permission, always loaded (Claude), easy to refer to, agents.md for the whole system at ~ top | `~/AGENTS.md` (chmod 444) · `~/.claude/CLAUDE.md` import · 🗄 button + `concourse cabinet` |
| 16 | Zig/Rust/Elixir heavily over Python; Python = POC only | hub is Rust · `~/AGENTS.md` §3 · audit flags Python tools |
| 17 | GTK4 & co. treated like Python — steer away | hub is egui · `~/AGENTS.md` §3 |
| 18 | Custom-compiled apps installed/viewable/callable as system packages | `make install`: PATH binary + .desktop + hicolor icons; rpm metadata ready |
| 19 | Direct developer repos over system packages, when maintainable | `~/AGENTS.md` §3 |
| 20 | Never "simply run this shell command" — do the work | everything installed and verified this session |
| 21 | Periodic "what am I not confident about?" checks → serious TODOs | docs/SERIOUS-TODOS.md (live) |
| 22 | Keep track of every unique ask | this file · `concourse asks` ledger for the system |
| 23 | Security: high-trust, fortress, full sudo, escalate for clarity | `~/AGENTS.md` §5 |
| 24 | (Standing, from the station) one pattern every tool speaks; registry claims carry their check | envelope/exit codes dogfooded · `doctor` re-checks live |
| 25 | (Standing, house rule 7) essential CLI actions get buttons | ⟳ REFRESH stone · per-card open/probe · cabinet/skills/asks/doctor buttons |
| 26 | Integrate more CLI-JSON between Nexus (Hermes skill) and the local AI setup — diffusion, models, STT, TTS — with communication between them (2026-07-07) | **intercom** (say/hear, Rust) + piper & whisper.cpp engines · voice-models.json registry · pipelines in `intercom schema` + both skills (`station/intercom` ↔ `intercom`) · registered on the board |
| 27 | Skills rack: can't resize down, glitchy after expand — fix; folder icon on the left, clickable, for the skill categories (2026-07-07) | window given real height + fill-scroll content (vertical drag verified under Xvfb) · RACKS pane: 📁 opens the category folder, name filters the list |
| 28 | `~/AGENTS.md` deleted (ugly in home) — remake it leaner in ClaudeWorkspace, Ben-user-only edit; let Nexus know via one line in her AGENTS.md pointing at the project dir, she forms her own opinions (2026-07-07) | rewritten ~40 % shorter at `~/Dev/ClaudeWorkspace/AGENTS.md`, sealed 444 · one pointer row in `~/Nexus/AGENTS.md`'s attention table · CLAUDE.md import, registry, doctor, cabinet verb + button all re-pointed |
| 29 | Everything looks the same — show the bays as a top-down folder structure with attention flow between docs: orange claude / pink nexus / grey opencode+misc, `-->` weak one-way, `<-->` mutual, brightness by connection volume, no archives; collapsible left control panel (view options, hide folders w/ grey unnamed ghosts, connections panel); sqlite ok; workflow doc + manually-called agent updates the index; open-in-terminal buttons (2026-07-07) | 🧭 estate view: SQLite index (`concourse attention scan`, 8k docs/24k links ≈ 5 s) · canvas map w/ bezier arrows, dynamic intensity, mutual double-heads · control panel (reindex, subdirs, only-connected, ghosts, per-folder hide, agent legend, connections list + edge inspector) · folder popouts with TERMINAL button · `docs/ATTENTION-WORKFLOW.md` |
| 30 | See systemd services/tasks/execution with enable/disable that prompts claude/hermes/opencode (selectable, remembers default), agent spawned programmatically, reports back via a standard CLI command flipping the UI green/red, agent logs clickable (2026-07-07) | ⚙ services window (user+system scopes, filter, enablement states) · dispatch spawns the chosen harness headless, transcript captured · `concourse job report <id> ok\|fail --note` is the wire · green ●/red ✗ with hover notes, log one click · default harness remembered in ui.json · verified live: self-test dispatch → claude investigated, refused to fabricate, reported fail honestly |
