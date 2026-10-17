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

Measured 2026-10-17, after the first burn-down pass (chisel_coverage module +
formcalc language edges):

| File | Lines | Missed | Line cover |
| --- | --- | --- | --- |
| `formcalc.rs` | 1909 | 236 | 87.64% |
| `lib.rs` | 846 | 96 | 88.65% |
| `xfa.rs` | 1055 | 107 | 89.86% |
| `xfa_session.rs` | 90 | 13 | 85.56% |
| **TOTAL** | **3900** | **452** | **88.41%** (was 87.00%) |

Closed so far (+45 lines): document object model tour (numFields,
getNthFieldName, pageNum, documentFileName, info, all five field-kind type
names), property setters/read round trips (required/hidden/readonly/
textColor/fillColor), setFocus, option surfaces (numItems/getItemAt flag/
currentValueIndices/isBoxChecked/checkThisBox/charLimit/exportValues),
instance managers (removeInstance/setInstances + the `in` operator on the
xfa proxy), oversized-script refusal, formcalc language edges (do..end,
scientific notation, NaN/Infinity/-Infinity formatting, unterminated
do-block errors, root refs).

Known-advisory CI facts (not regressions):
- `workspace-gate / coverage` fails on CI runners (instrumented build of the
  whole 33-crate workspace exceeds the runner's comfort locally it passes in
  ~4 min); tier c is informational by definition. Infra ticket: split or
  budget the workspace coverage job.
- `gate-parser-js / coverage` needs ~62 more covered lines to reach 90%.

Remaining targets, in order of value:
1. `lib.rs` (+54 needed): the remaining field-native functions — draw/button
   behaviors, console object, doc-level `mailDoc`/`submitForm` request paths.
2. `formcalc.rs` (+56): the recursive-descent tail — trailing-token errors
   from `run_formcalc_at`, list-valued conversions, `_child` instance
   managers of nested rows.
3. `xfa_session.rs` (+13): injected engine-failure paths (disconnected
   channel, start timeout) — needs a wedged-engine test fixture.
4. `xfa.rs` (+2 to hit 90%): one more fixture node kind (draw or area).

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
