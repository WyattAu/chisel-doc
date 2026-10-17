# ROADMAP — Chisel Docs

The upstream roadmap remains in git history (`upstream/main: ROADMAP.md`).
This fork's direction is the Chisel program's 90-day plan; summary:

## Done (week 1–2)

- [x] Fork + clean-room decision record (services excluded by licence)
- [x] Rebrand sweep: upstream marks removed outside the attribution allowlist,
      CI-enforced (`rebrand-gate`); placeholder brand artwork in `assets/brand/`
- [x] Estate gate matrix wired (`.github/workflows/estate-gate.yml`) — advisory
      on inherited code, blocking rebrand/no-secrets gates
- [x] Build validation: `cargo check --locked --workspace` green

## Weeks 3–6 — hardening gates

- [ ] Tier-A scoped gates graduate advisory → blocking for `pdfcraft-cos`,
      `pdfcraft-js` (clippy pedantic burn-down, coverage ratchet)
- [ ] cargo-fuzz harnesses on parser entrypoints (COS lexer, xref, streams,
      font loaders); CI smoke budget + nightly deep runs
- [ ] loom models on page/state caches; miri triage of unsafe in decoders
- [ ] cargo-vet enforcing with committed `supply-chain/`
- [ ] First provenance-attested release (SHA256SUMS + provenance JSON +
      signature), replacing SHA256SUMS-only
- [ ] Vendor drift-checks for `vendor/` (hayro, egui-wgpu)

## Weeks 7–10 — platform slice 1

- [ ] `chisel-platform`: OIDC/PKCE + passkey auth, JWT sessions, RBAC
- [ ] Content-addressed asset store (BLAKE3, verify-on-read) behind the app's
      team library
- [ ] Observability: opt-in OTLP in-app, always-on service metrics, real
      health checks

## Weeks 11–13 — platform slice 2

- [ ] Job orchestration (outbox + idempotency + worker pools) for
      render/redaction/batch
- [ ] Tamper-evident document audit trail extending `pdfcraft-audit`, local +
      server-side, query API, in-app Audit panel
- [ ] Committed latency budgets (criterion + percentile tracking, CI-regressed)

## Standing upstream-first track

Fuzz harnesses, corpus additions, and bug fixes found by our gates go upstream
to storytold/pdfcraft as PRs whenever they are fork-agnostic. Weekly upstream
merge cadence; conflicts triaged same day.
