# Ratchet baseline — measured 2026-10-17

Estate doctrine: inherited code runs **advisory** and graduates to blocking
once green (engineering-standards LOOP.md). This file is the starting point of
that burn-down — the red-list, measured on the fork's first public CI run, with
a target per item. Nothing here is aspirational; every number came out of a
local run matching what CI executes.

## Gate matrix status (first run on `main`)

| Job | Scope | Result | Note |
| --- | --- | --- | --- |
| `rebrand-gate` | blocking | **pass** | legal requirement, day-one green |
| `workspace-gate / fmt` | c | pass | failed once (28 import lists after the `ChiselDocsApp` rename); fixed |
| `workspace-gate / clippy` | c | **pass** | `-D warnings` clean across the workspace |
| `workspace-gate / check, test, layer, feature-compat` | c | pass | |
| `gate-parser-cos / clippy` | a | **was failing** | shared-workflow bug, since fixed in engineering-standards (PKG_FLAG after `--`) |
| `gate-parser-js / clippy` | a | **was failing** | same cause |
| `gate-parser-cos / coverage` | a | pass | **91.03%** line ≥ 90% Tier-A bar |
| `gate-parser-js / coverage` | a | **fail: 82.28%** vs 90% required | burn-down target below |
| `workspace-gate / coverage` | c | informational (≥70%) | not yet printed to completion |

## The one real gap: `pdfcraft-js` coverage

```
TOTAL   9859 lines   1747 missed   82.28% line   82.08% region   87.00% func
```

Weakest files (line coverage):

| File | Line | Region |
| --- | --- | --- |
| `lib.rs` | 81.18% | 73.28% |
| `formcalc.rs` | 81.03% | 88.55% |
| `xfa.rs` | 85.49% | 82.20% |
| `xfa_session.rs` | 83.80% | 76.92% |

**Burn-down plan** (each step lands as its own commit, gate graduates when the
whole list is done and the loop has been green):

1. `lib.rs` — the `run()` dispatch surface: every `app.*` / `util.*` /
   `display.*` entry point gets one test each. Cheapest lines-per-hour here.
2. `xfa_session.rs` — XFA session lifecycle (region coverage 76.92% is the
   worst number anywhere in the crate; this is a real behaviour gap, not just
   untested lines).
3. `formcalc.rs` — the calculate/keystroke expression evaluator: table-driven
   cases per operator, plus the Acrobat-JS edge semantics the fuzz harness
   reaches but tests do not.
4. `xfa.rs` — XFA template fill paths.

Target: ≥90% line **and** region on `pdfcraft-js`; then promote
`gate-parser-js / coverage` to a required check.

## Clippy

Clean today at `-D warnings` for the workspace and for both parser crates
separately. The Tier-A pedantic pass is the next burn-down and needs no
tickets — new findings are simply not allowed in.

## Vet / deny / semver / fuzz / vendor

- `deny`: upstream's `deny.toml` is in force and passing (permissive-only,
  sources-gated, two reasoned advisory exemptions).
- `vet`: `cargo vet init` produced `supply-chain/` (`config.toml`,
  `audits.toml`, `imports.lock`); the gate reports remaining coverage gaps,
  which the resync checker turns into actionable exemptions. Not yet burning
  them down — the first `cargo vet --locked` result is the list.
- `semver`: no tags yet on the fork, so the baseline begins at the first
  release (by design; see BRANCH-PROTECTION.md).
- `fuzz`: three harnesses, 59 309 cumulative runs, zero crashes.
- `vendor ledger`: 356 citations verified, 0 findings; the six hermetic
  vendored crates' suites run in CI.
