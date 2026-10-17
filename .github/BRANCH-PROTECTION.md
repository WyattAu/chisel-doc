# Branch protection — `main`

Set with `gh api` on 2026-10-17 (see the command block at the bottom). This file
is the record of *why* each check is or is not required.

## The advisory problem

Estate doctrine is that inherited code runs **advisory** and ratchets to
blocking once green (engineering-standards LOOP.md). In GitHub, "advisory" is a
**branch-protection** property, not a workflow property:

> A job that calls a reusable workflow (`uses:`) may not set
> `continue-on-error` — GitHub rejects the key at workflow-load time and the
> whole workflow fails in 0s with no jobs. Observed here on 2026-10-17.

So the ratchet is expressed by the **required-checks list** below, not by flags
in the YAML. A check graduates from advisory to blocking by being added here,
and its `TODO(ratchet)` comment in the workflow is deleted at the same time.

## Required now (blocking)

| Check | Workflow | Job |
| --- | --- | --- |
| CI | `ci.yml` | `test` (ubuntu, windows), `wasm` |
| Rebrand gate | `estate-gate.yml` | `rebrand-gate` |
| Asset policy | `ci.yml` | `test` → `cargo xtask assets` |

The rebrand gate is blocking from day one: it is cheap, deterministic, and it
is a **legal** requirement (upstream's terms demand removal of their marks), so
there is no burn-down argument for it.

## Advisory (not required)

| Check | Workflow | Graduates when |
| --- | --- | --- |
| `workspace-gate` (tier c) | `estate-gate.yml` | coverage ≥ 70% line and clippy pedantic clean |
| `gate-parser-cos` (tier a) | `estate-gate.yml` | ≥ 90% on the parser crate + fuzz smoke green for a full loop |
| `gate-parser-js` (tier a) | `estate-gate.yml` | same, for the JS sandbox |
| `fuzz smoke` ×3 | `fuzz.yml` | 30 consecutive green nightly deep runs |
| `vendor ledger` | `vendor-patches.yml` | green for a full loop (it is at 0 findings today) |
| `vendor tests` ×6 | `vendor-patches.yml` | green for a full loop |

Per the estate's own graduation rule, a gate lands advisory for one loop and
then becomes blocking — the review is recorded in the commit that adds the
check here, not in the workflow file.

## Other protections

- force-push: disabled
- deletion: disabled
- required approvals: 0 (solo program; revisit on the first collaborator)
- linear history: **not** required — the fork carries upstream's merge history,
  and forcing linearity would rewrite it
- conversation resolution: required

## Re-applying

```sh
gh api -X PUT repos/WyattAu/chisel-doc/branches/main/protection \
  -F required_status_checks[strict]=false \
  -f 'required_status_checks[contexts][]=CI / test (ubuntu-latest)' \
  -f 'required_status_checks[contexts][]=CI / test (windows-latest)' \
  -f 'required_status_checks[contexts][]=CI / wasm32 check' \
  -f 'required_status_checks[contexts][]=estate-gate / rebrand-gate' \
  -F enforce_admins=false \
  -F required_pull_request_reviews[required]=false \
  -F restrictions=null \
  -F required_linear_history=false \
  -F allow_force_pushes=false \
  -F allow_deletions=false \
  -F required_conversation_resolution=true
```

Check names drift with matrix/job renames — read the live list with
`gh api repos/WyattAu/chisel-doc/branches/main/protection/required_status_checks`
and reconcile before re-applying this block.
