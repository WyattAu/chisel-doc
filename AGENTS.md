# AGENTS.md — rules for AI agents and contributors (Chisel Docs fork)

Derived from upstream PdfCraft's contributor rules, forked and tightened for
the Chisel program. Section numbering inside §1 is preserved because code and
tooling reference it (`xtask/src/assets.rs`, `xtask/src/screenshots.rs`).

## §1 Asset policy

### §1.1 Adobe-authored technical data

The closed list of Adobe-authored, non-visual technical data is unchanged from
upstream: `ADOBE_DATA_ALLOWED` in `xtask/src/assets.rs` (hayro cmaps, font
metrics) and `ADOBE_DATA_FILES` (`crates/fonts/src/std14_metrics.rs`). Each
needs an `[[asset]]` entry with `adobe_data = true` so its SHA-256 is
attested. Do not add new Adobe-authored files.

### §1.2 Brand assets

Brand marks live in `assets/brand/` with licence `LicenseRef-Chisel-Trademark`
(terms: `assets/brand/LICENSE-brand.txt`). They are **not** open source, may
be used only unmodified within Chisel, and forks must remove them. Only
`kind = "logo"` entries may use that licence (enforced by `cargo xtask
assets`). Current artwork is PLACEHOLDER pending name screening — see
`REBRAND.md`.

### §1.3 Everything else

Images, icons, fonts-by-value and other assets carry per-file entries in
`ATTRIBUTION.toml`. `ATTRIBUTION.md` is **generated** — never edit it by hand;
run `cargo xtask assets --write` after manifest changes and commit both.

### §1.4 Fonts

Never commit a font file. Optional Japanese/embed fonts come from the
`craft-fonts` build input via `CRAFT_FONTS_DIR` (see `CLAUDE.md`); the Latin
UI fonts already in `assets/fonts/` stay.

### §1.5 Vendored patches

The crates under `vendor/` (hayro stack, lopdf, winit, egui-wgpu, vello_common)
carry numbered patches recorded in `vendor/README.md`: rationale, upstream PR
link where one exists, and the regression test that proves each patch. Every
patch is marked `PdfCraft patch:` in the source and is meant to go upstream;
remove a vendored copy once upstream ships the fix. Never vendor copyleft
(plan/adr/0001).

`scripts/check-vendor-ledger.py` verifies the ledger against the sources —
every cited test resolves, versions match manifests, every `[patch.crates-io]`
crate is listed — and `.github/workflows/vendor-patches.yml` additionally runs
the vendored crates' own unit suites (the ledger's proof column) and compiles
the two whose tests need live sessions or a GPU. When you add a patch: marker
in source, numbered entry with rationale and test, ledger green.

## Never crash

Unchanged from upstream, and it still outranks features: a malformed PDF, bad
tool argument, corrupt settings file or full disk must produce an actionable
error — never a panic, never lost work. No `unwrap()`/`expect()` outside
tests; `panic = "abort"` policy applies to the shipped binaries.

## Fuzz policy

Every public entry point that takes bytes or text from an untrusted PDF has
a libFuzzer harness asserting Err-not-panic, with resource amplification
capped so a run stays fast (estate standards §4):

| Harness | Crate | Target |
| --- | --- | --- |
| `pdfcraft-filters-fuzz` | filters | `decode` (stream decoders, output bound asserted) |
| `pdfcraft-cos-fuzz` | cos | `parse_indirect` (object layer, length-resolver branches), `lexer_object` (recursive object lexer) |
| `pdfcraft-js-fuzz` | js | `run` (Acrobat-JS sandbox, tightened `Limits`) |

Harnesses live in `crates/<crate>/fuzz/` (detached workspaces, `[package.metadata]
cargo-fuzz = true`) and run with `cargo +nightly fuzz run <target>` from the
crate directory. `.github/workflows/fuzz.yml` smoke-runs them per PR
(advisory until the ratchet) and deep-runs 15 minutes nightly.

Two other fuzzers cover this repository and are **not** replaced by the above:
`cargo xtask fuzz` (stable-Rust mutation driver: whole-document open, render,
edit, save, reopen, scheduled in `ci.yml`) and `cargo xtask corpus`. A crash
artifact is a finding, not a deliverable: minimize it, distill a synthetic
regression test into the crate's test suite, and — for parser bugs — send the
harness and the regression upstream as a PR.

## Clean-room boundary (hard rule)

Contributors may read public specs (ISO 32000, public corpus documentation)
and upstream PdfCraft source. **Nobody reads or copies
`storytold/artcraft-services`** — it is licensed under a custom fair-source
licence incompatible with this fork, and its code must not influence Chisel's
service layer.

## CI gates

- `estate-gate.yml`: the WyattAu standards matrix (locked build, clippy -D
  warnings, deny/vet, coverage, semver, scoped Tier-A parser gates) —
  advisory for inherited code, ratcheting to blocking.
- `rebrand-gate` (blocking): no upstream marks outside
  `.rebrand-allowlist.txt`; no committed secrets, ever.
- Upstream workflows (ci.yml, release.yml, platform builds) still run; their
  org targets are placeholders pending the platform move — do not "fix" them
  back to upstream.

## Naming

Internal crate names are still `pdfcraft-*` (unpublished identifiers — rename
deferred by decision; do not rename opportunistically). User-facing strings
use **Chisel Docs**. App id: `com.wyattau.chisel.doc`. If you change an English
UI source string that contains the app name, mirror it in every
`crates/ui-egui/src/i18n/*.tsv`.

## Contributions

DCO sign-off (`git commit -s`) is required; no CLA. Security issues: see
`SECURITY.md` upstream policy retained for the engine; program-level contact
is the Chisel repo security page.
