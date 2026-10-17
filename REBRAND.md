# REBRAND — fork sweep to remove upstream brand marks

Legal basis: upstream `NOTICE` — "The ArtCraft name, wordmark and logos in
docs/brand/ are trademarks of the ArtCraft Team. They are not open source and
are not covered by the license above; forks and modified versions must remove
them." Apache-2.0 §6 independently grants no trademark rights.

**Enforcement:** the `rebrand-gate` job in `.github/workflows/estate-gate.yml`
(blocks CI) greps for `artcraft|storytold|storyteller|getartcraft` and fails on
any hit outside `.rebrand-allowlist.txt`. The allowlist is legal attribution
and dependency provenance only. **This gate is the definition of done.**

## Measured inventory (2026-10-17, shallow clone of upstream/main)

| Mark | Files containing it |
| --- | --- |
| `artcraft` | 48 |
| `storytold` | 45 |
| `storyteller` (incl. `ai.storyteller.*` ids) | 23 |
| `getartcraft` | 8 |

Hits by area: `crates/` 38 · `packaging/` 14 · `docs/` 13 · `.github/` 7 ·
`xtask/` 4 · `apps/` 4 · `assets/` 2 · `vendor/` 1 · root files 12
(README, ROADMAP, AGENTS.md, CLAUDE.md, NOTICE, LICENSE-MIT, deny.toml,
Cargo.{toml,lock}, ATTRIBUTION.{md,toml}, contributors/).

Physical brand assets (delete/replace):

```
docs/brand/artcraft-logo{,-white}.{png,svg}       4 files — DELETE
docs/brand/artcraft-mark{,-black}.{png,svg}       4 files — DELETE (+ LICENSE-brand.txt)
assets/app-icon/hicolor/*/apps/ai.storyteller.pdfcraft.{png,svg}   9 files — REPLACE + RENAME
packaging/linux/ai.storyteller.pdfcraft.{desktop,metainfo.xml.in,mime.xml}   3 — RENAME + rewrite
packaging/linux/flatpak/ai.storyteller.pdfcraft{.bundle.yml,.yml}            2 — RENAME + rewrite
```

Secrets check: **clean** — no `.env`/`secrets`/credential files (verified; the
estate-gate job now also blocks them permanently).

## Naming tokens (pending trademark screening — see plan doc 02)

| Token | Value |
| --- | --- |
| Product/app id | `com.wyattau.chisel.doc` (placeholder until screening passes) |
| Display name | `Chisel Docs` |
| Binary names | `chisel-doc`, `chisel-doc-cli` |

## The sweep (scripted, in order)

1. **Delete** `docs/brand/` entirely (incl. `LICENSE-brand.txt`).
2. **Icons:** regenerate/rename `assets/app-icon/hicolor/**/apps/` entries to
   `com.wyattau.chisel.doc.png|svg`; update any icon-path constants in
   `crates/ui-egui` / `apps/pdfcraft`.
3. **Packaging:** rename the five `packaging/linux/ai.storyteller.pdfcraft.*`
   files to `com.wyattau.chisel.doc.*`; rewrite ids, Names, Icons, MimeType
   references, flatpak `app-id`/`command`; check `packaging/` for
   macOS/Windows bundle ids too (`grep -rn "ai.storyteller" packaging/`).
4. **Config/cache paths:** upstream stores profiles under
   `~/.config/ai.storyteller.*`-style dirs — switch to
   `~/.config/com.wyattau.chisel.doc/`. Deliberately **no** migration from old
   paths: different product, fresh profile. Note in release notes.
5. **User-facing strings:** "PdfCraft" → "Chisel Docs" in About, window titles,
   CLI `--help`, crash messages, `packaging/` summaries, metainfo
   name/description, website/URLs (remove getartcraft/Discord links — Chisel
   has its own).
6. **Workflow files:** `.github/release.yml` and friends push to upstream
   repos/app-ids — neutralise or retarget; `freebsd.yml`, `nix.yml`,
   `windows-arm64.yml` keep but verify no upstream upload targets.
7. **Docs rewrite:** README.md, ROADMAP.md, AGENTS.md, CLAUDE.md, `book/`,
   `docs/` — rewrite for Chisel; keep upstream attribution alive via NOTICE
   only.
8. **Run the gate locally:**
   ```sh
   grep -riE 'artcraft|storytold|storyteller|getartcraft' \
     --exclude-dir=.git --exclude-dir=target \
     $(awk '!/^#/ && NF {print ($1 ~ /\/$/ ? "--exclude-dir=" substr($1,1,length($1)-1) : "--exclude=" $1)}' .rebrand-allowlist.txt) \
     .
   ```
   Must output nothing.

## Deferred (tracked, not blocking)

- **Internal crate renames** `pdfcraft-*` → `chisel-*`: deferred. They are
  unpublished workspace identifiers, not marks; renaming all 33 crates now
  maximizes merge friction against upstream for zero legal gain. Revisit at
  first public release. (The gate greps org marks, not `pdfcraft`.)
- **Upstream domain strings in test fixtures** (`crates/*/tests/**` corpora may
  embed `artcraft` in fixture content): if the gate trips on fixtures, allowlist
  the fixture path with a comment — fixture data is evidence, not branding.

## After the sweep

Open the fork PR with the sweep as the first commit series; then the name
screening verdict lands in plan doc 02 and tokens above get finalized.
