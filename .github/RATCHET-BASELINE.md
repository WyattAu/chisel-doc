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
| **TOTAL** | **3900** | **436** | **88.82%** (was 87.00%; lib.rs alone 90.43%) |

Closed so far (+71 lines net): document object model tour (numFields,
getNthFieldName, pageNum, documentFileName, info, all five field-kind type
names), property setters/read round trips (required/hidden/readonly/
textColor/fillColor), setFocus, option surfaces (numItems/getItemAt flag/
currentValueIndices/isBoxChecked/checkThisBox/charLimit/exportValues),
instance managers (removeInstance/setInstances + the `in` operator on the
xfa proxy), oversized-script refusal, formcalc language edges (do..end,
scientific notation, NaN/Infinity/-Infinity formatting, unterminated
do-block errors, root refs).

Second wave landed (radiobutton kind, app dialogs, formcalc parse errors).
Skipped by design: the JS-side `xfa` proxy instance-manager methods and the
session engine-failure paths — both need designed fixtures (a real xfa-node
JS fixture; a wedged-engine injection) rather than guesses, and each guess
cost a compile cycle without landing coverage.

Third wave landed 2026-10-21 (+99 lines net, measured): **the 90% line gate is
crossed**. `util.printd` (every picture branch + the numeric shorthand codes +
the NaN-date refusal), `util.printx` (all format characters, case directives,
escapes, exhausted-source arms), `util.printf` (space/hash/zero flags +
`,2`/`,3` grouping), the field-native tails (text kind name, multi-value
listbox reads, checkbox Off-filtering on write, uncheck clears, fillColor
writes, `submitForm` with a string URL, the global no-ops
`mailDoc`/`calculateNow`/`syncAnnotScan`/`dirty`, object-form `app.alert`),
and on the FormCalc side the financial tail (`Term`/`CTerm`/`Npv`/`Apr`/`Ipmt`
bisection + guards), `DateFmt` styles, `Num2Date` pictures, `Num2Time` null,
loop `break` in both directions + non-finite bounds, the 1 MiB string cap
(lexer unit test — the script cap would mask it end-to-end), tens-exact number
words, and `Concat` results past the per-result string cap.

**Measurement note (2026-10-21):** an accumulated `llvm-cov-target/` that had
seen `--all-features` and no-features builds across sessions silently added
~237 phantom missed lines (local 89.42% vs CI 92.11% on identical code).
`cargo llvm-cov clean` before measuring; CI is ground truth. Authoritative
per-CI numbers at the gate-crossing commit: **line 92.11%, region 87.10%,
functions 88.43%** (3 864 lines / 305 missed). The per-file line table in
older snapshots below was measured the same flawed way — trend valid,
absolute values conservative.

Known-advisory CI facts (not regressions):
- `workspace-gate / coverage` fails on CI runners (instrumented build of the
  whole 33-crate workspace exceeds the runner's comfort locally it passes in
  ~4 min); tier c is informational by definition. Infra ticket: split or
  budget the workspace coverage job.
- `gate-parser-js / coverage` is over the 90% line bar as of this wave; the
  gate should turn green on the next CI run. Graduation to a required check
  still awaits region coverage ≥ 90% and a green loop.
- `workspace-gate / semver` fails inside cargo-semver-checks itself: the
  tool's generated manifest drops the `[patch.crates-io]` table, so its
  rustdoc build of `pdfcraft-render` resolves crates.io `hayro` (no
  `render_into`/`ColorReplacement`) instead of the vendored stack and
  aborts with E0432/E0433 — an upstream tooling limitation, not an API
  break (`--baseline-rev chisel-v0.1.0-alpha` reports "no semver update
  required" for every crate it can build). The scoped per-crate semver
  gates (`gate-parser-js`, `gate-parser-cos`) are green. Upstream issue
  material; revisit when cargo-semver-checks honours workspace patches.

Remaining targets (clean-instrumentation numbers, 2026-10-21; the line bar is
met at 92.11% — this is the graduation tail: region 87.10% → 90 needs ~114
regions, functions 88.43% → 90 needs ~6):
1. `formcalc.rs` (740 missed regions, 169 missed lines): the recursive-descent
   tail — trailing-token errors from `run_formcalc_at`, list-valued
   conversions, `_child` instance managers of nested rows.
2. `xfa.rs` (374 regions / 107 lines): the JS-side proxy methods
   (item/insert/moveInstance/count) — needs the designed xfa-node JS fixture.
3. `lib.rs` (365 regions / 128 lines): cfg(not(wasm)) engine-start error paths
   and the platform-name arms (`MAC`/`WIN` are runner-dependent by nature).
4. `xfa_session.rs` (26 regions / 13 lines): injected engine-failure paths
   (disconnected channel, start timeout) — needs a wedged-engine test fixture.

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

## Trail-on-wasm32 status (2026-10-18)

The `trail` feature stays **feature-gated** (native-only). Flipping it
default-on exposed a uuid 1.26.1 feature-resolution gap: tamper-audit 0.2.1
declares `uuid = { features = ["rng-getrandom"] }` in its wasm32 target
table, the `rng` umbrella unifies into the wasm graph, but uuid's own
compile-time check requires the *specific* `rng-getrandom`/`js`/`rng-rand`
flags and does not see them through the transitive target table + feature
unification. The wasm gate fails on uuid (and tokio's non-wasm feature set
via the same new edge) even though every individual manifest is correct.

Reverted to the green posture: trail feature-gated (native), wasm gate 31
crates ok. The flip re-opens when either (a) tamper-audit adopts an
explicitly wasm-safe uuid configuration validated in ITS wasm CI and
documented for consumers, or (b) chisel-doc hosts the audit trail behind a
service API on the web build (the server continuation design), removing
the in-crate trail from the wasm surface entirely. Option (b) is the
architecturally correct end state for the web target.
