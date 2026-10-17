# Chisel Docs

**An industrial-hardening fork of [PdfCraft](https://github.com/storytold/pdfcraft) —
a PDF reader and editor written from scratch in pure Rust.**

> **Status: pre-alpha fork, in rebrand.** The product name is a placeholder
> pending trademark screening. Every legal-attribution file (NOTICE,
> ATTRIBUTION, LICENSE-*) preserves the upstream project's terms verbatim.

## What this fork is

Chisel Docs takes upstream PdfCraft's engine (33-crate pure-Rust workspace:
`cos` object system, `render`, `sign`, `redact`, `audit`, OCR, forms) and
wraps it in a defence-grade engineering program:

- **Enforced gate matrix** (`.github/workflows/estate-gate.yml`): locked
  builds, clippy `-D warnings`, cargo-deny/vet, coverage tiers,
  cargo-fuzz/miri/loom on untrusted-input parsers, per-release provenance.
  Inherited code runs advisory and ratchets to blocking; see
  `.github/workflows/estate-gate.yml` for the current posture.
- **Rebrand gate**: zero upstream brand marks outside the attribution
  allowlist (`.rebrand-allowlist.txt`), enforced in CI. See `REBRAND.md`.
- **Platform track** (separate `chisel-platform` repo): OIDC/passkey auth,
  content-addressed asset store, tamper-evident audit trails, render-job
  orchestration, opt-in telemetry with committed latency budgets.

## Build

Same as upstream — the toolchain is unmodified:

```sh
git clone <this repository>
cd chisel-doc
cargo check --locked --workspace
cargo run --release -p pdfcraft -- file.pdf
cargo test --workspace
```

Japanese fonts for tests/embedding come from the optional `craft-fonts` build
input (see `CLAUDE.md`).

## Provenance and licence

This repository derives from the ArtCraft project's PdfCraft, dual-licensed
MIT OR Apache-2.0. The derivation is Apache-2.0. Required notices:
[NOTICE](NOTICE) · [ATTRIBUTION.md](ATTRIBUTION.md). The upstream project and
its apps are unrelated to, and do not endorse, this fork.

Program documentation (licensing audit, naming, platform design, 90-day plan)
lives outside this repository in the Chisel program workspace.
