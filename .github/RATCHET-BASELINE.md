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
- ~~`workspace-gate / coverage` fails on CI runners~~ **RESOLVED 2026-10-21** —
  three shared-workflow fixes landed in engineering-standards: `-j 2`
  (instrumented build OOM at default parallelism), the caller's `all-features`
  input honoured (a hardcoded `--all-features` flipped GPU-dependent tests on),
  and `-- --test-threads=1` + a `coverage-exclude` input (the ui-egui content
  tests' kittest wgpu path cannot create an adapter on a plain runner, serial
  or not — excluded from the informational workspace number; the regular test
  job still runs them). Green in run 39620217269. Also fixed en route: the
  test-arg passthrough must stay the last thing on the llvm-cov line (a flag
  appended after `--` becomes a test-binary option — observed as
  "Unrecognized option: 'p'").
- `gate-parser-js / coverage` line gate is green; region 88.97% and functions
  91.81% after waves four through six (wave six: the wedged-worker test —
  a cfg(test) sentinel makes the worker exit before a marked script, proving
  the documented fault-in-engine thread replacement). Graduation to a
  required check still awaits region ≥ 90% and a green loop.
- ~~`workspace-gate / semver` fails inside cargo-semver-checks~~ **worked
  around caller-side 2026-10-22** — the tool's generated manifest drops the
  `[patch.crates-io]` table, so the rustdoc build of the render/hayro cone
  resolves crates.io `hayro` and aborts (E0432/E0433) — an upstream tooling
  limitation, not an API break (`--baseline-rev chisel-v0.1.0-alpha` reports
  "no semver update required" for every crate it can build). The shared
  workflow gained a `semver-exclude` input; the workspace scan excludes the
  render/hayro cone (render, engine, automation, cli, optimize, ui-egui,
  pdfcraft, web) and checks every other crate. First fully-green
  estate-gate run: 39712791657 (30/30). The real fix stays tracked upstream:
  [obi1kenobi/cargo-semver-checks#1777](https://github.com/obi1kenobi/cargo-semver-checks/issues/1777);
  when it lands, the excludes are deleted and the cone rejoins the scan.

Remaining targets (clean-instrumentation numbers, 2026-10-24; line 93.77%,
functions 91.81%, region 88.97% after wave six — the graduation tail is ~107
regions of genuinely hard remainder):
1. formcalc.rs (651 regions / 145 lines): the token describer's number arm is
   defensive dead code (numbers always start statements); the rest is SOM
   grammar — `..` descendants, list-valued member productions, nested gather
   arms — each needs a purpose-built fixture, one region at a time.
2. xfa.rs (289 regions / 71 lines): the remaining proxy arms past the swept
   item/insert/move/count surface (live-list rebuilds, effect clipping
   counters, refuse-logic arms).
3. xfa_session.rs (16 lines, 91.35% after wave six): the send-failure arm
   (worker to sender dropped between scripts) is defensive — this worker
   design exits only while receiving, so the session's next send always
   reaches either a live or a freshly spawned worker (proven by the
   replacement test). lib.rs (128 lines): the engine-start error paths need
   the same cfg(test)-seam treatment the session got.
4. Decision needed at the ratchet review: region 90% may be the wrong bar for
   a crate whose remaining uncovered regions are dominated by defensive
   depth — the estate alternative is `coverage-ignore-regex` on the documented
   defensive surfaces (the input exists; template ADR-0005 is the precedent).

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
