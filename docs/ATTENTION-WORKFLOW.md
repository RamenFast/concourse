# The attention workflow — updating the estate's link index

*For any agent Ben calls by hand — Claude Code, Hermes/Nexus, or opencode — from a terminal
in this directory (the estate map's "open in terminal" buttons land you here or in a bay).
One job: refresh the index the estate map draws from, and say honestly what changed.*

## What the index is

`~/.local/share/concourse/attention.db` (SQLite) records every `.md`/`.html` doc under the
estate roots and every reference between them — markdown links, `[[wikilinks]]`,
`href`/`src`, and bare `~/…` paths. The Concourse GUI renders it as the estate map:
top-down folder structure, directional arrows for attention flow.

- **Roots:** `~/Dev/ClaudeWorkspace` · `~/Nexus` · `~/.claude/skills` + `CLAUDE.md` ·
  `~/.hermes/skills` + `SOUL.md` · `~/.config/opencode` (the list lives in
  `src/attention.rs::ROOTS`).
- **Never indexed:** anything archived (any dir whose name contains "archive"), dot-dirs,
  `node_modules`/`target`/build trees. This is Ben's law — archives are memory, not attention.
- **Colors on the map:** orange = Claude's home turf, pink = Nexus's, grey = opencode and
  misc/unknown. `-->` one-way (weaker line) · `<-->` mutual (stronger); brightness scales
  with connection count.

## The job

1. Rebuild: `concourse attention scan --json`
   — deterministic, no LLM judgement involved; ~5 s for the whole estate.
2. Read the envelope. `status: ok` plus sane counts (thousands of docs, not zero or double).
   If docs collapsed toward zero, a root moved — check `src/attention.rs::ROOTS` against
   reality and fix the code, don't fake the data.
3. Spot-check: `concourse attention edges --json` — the top edges should look like the
   estate you know (PKM, skills, the dwelling). Name anything surprising in your report.
4. Report the delta in one or two sentences (docs before → after, link pairs before →
   after, anything odd). If you can't verify, say so — a claim carries its check.

The scanner is code, so if reality outgrows it (new estate root, a new doc format, a
reference style it misses), the fix is a small PR to `src/attention.rs` — propose it,
don't work around it.

## What not to do

- Don't hand-edit the database; `scan` rebuilds it wholesale anyway.
- Don't index archived material by "helpfully" widening the roots.
- Don't decide what's noise — hiding folders is Ben's call, made in the GUI's control panel.
