# SERIOUS-TODOS — the metacognitive ledger

*Ben's rule: while building, periodically ask "what am I not confident about?" — every honest
answer becomes a serious TODO. Founded 2026-07-07 during the first build. Items get struck
when resolved, never deleted.*

## Open

1. **GUI visual truth.** The hall was verified headless (Xvfb screenshot) on day one, but only
   in Blossom Dark at one window size. Walk all four themes at small/large sizes on the real
   desktop; check hairline contrast in Light and the flash animation's restraint. *(UI/UX)*
2. **Hermes skill count reads 176.** The depth-1 walk of `~/.hermes/skills/` counts nested
   category skills — verify against Nexus's own taxonomy that nothing is double-counted and
   nothing real is missed; her categories are hers, not guessable. *(behavior)*
3. **Human board columns drift with emoji-width names.** `{:<w$}` padding counts chars, not
   terminal cells; double-width glyphs (🗄 🏮) nudge columns in the TTY render. Cosmetic;
   fix with a width-aware pad if it bothers the eye. *(polish)*
4. **HTTP probe chunked-encoding handling is crude.** The hex-line filter works for today's
   loopback bodies (bridge/helper/ollama) but a chunked body containing a bare hex-looking
   line would mangle. Replace with a real chunked decoder or Content-Length-only read. *(robustness)*
5. **Refresh overlap.** Auto-refresh (120 s) and a manual REFRESH can overlap; probes are
   idempotent and the map write is last-wins, so harm is bounded — but a generation counter
   would make stale results drop cleanly. *(robustness)*
6. **sudoplz --json retrofit** is the standard's named first patch (§3 work list). Small,
   high-value; do it as its own wave with its own verify. *(estate work)*
8. **Registry seed vs config drift.** `assets/registry.json` (repo) seeds
   `~/.config/concourse/registry.json` once; later seed improvements won't reach an existing
   config. Teach doctor to diff seed-version vs config-version and say so. *(lifecycle)*
9. **Icon legibility at 24 px.** The arch-and-board SVG reads at 256; verify the small sizes
   in the panel/taskbar and thicken strokes if it muddies. *(polish)*
10. **`concourse run` and JSON mode.** `run` execs the node binary directly (stdio inherited),
    so `--json` before `run` applies to concourse's *own* errors only — document that the
    node's flags belong after the node id (e.g. `concourse run phosphor probe --json`). *(docs)*
11. **`run` passthrough can have GUI side effects.** Verifying exit propagation with
    `concourse run phosphor nonsuch` hit phosphor's bare-FILE fallback and *focused Ben's
    live scope once* (2026-07-07, logged as phosphor hole #3 in the standard). Lesson encoded:
    agents should pass explicit verbs through `run`, and hole #3's fix removes the trap.
    Exit propagation itself is verified verbatim (surveyor unknown-verb: 1 == 1). *(behavior)*

## Resolved

- ~~wisp binary missing while STATION-MAP said shipped~~ → rebuilt 2026-07-07 (`cargo build
  --release` in wisp/), map row now carries its check via `concourse probe wisp`.
- ~~phosphor branch sprawl (doctor finding #1, escalated)~~ → **Ben ruled 2026-07-07: leave
  them** — the 13 existing branches are grandfathered; the ≤1-branch law governs new work.
  The doctor's warn on phosphor is thus expected, not actionable.
