# CLAUDE.md — Chisel Docs (fork)

Read `AGENTS.md` first; it binds this repository. The short form for agents:

- Never crash on untrusted input; `unwrap`/`expect` only in tests.
- Never commit fonts or brand artwork without manifest entries; `ATTRIBUTION.md`
  is generated (`cargo xtask assets --write`), never hand-edited.
- Never read or copy `storytold/artcraft-services` (clean-room boundary,
  AGENTS.md).
- User-facing name is **Chisel Docs**; internal crates remain `pdfcraft-*` by
  decision — do not rename opportunistically.
- The rebrand gate (`.github/workflows/estate-gate.yml`) is blocking: no
  upstream marks outside `.rebrand-allowlist.txt`.

Optional fonts build input for tests (Japanese text, UI fallbacks):

```sh
git clone https://github.com/wyattau/craft-fonts ../craft-fonts   # placeholder org, see AGENTS.md
CRAFT_FONTS_DIR="$PWD/../craft-fonts" cargo test --workspace
```

Code using `pdfcraft_fonts::CRAFT_FONTS` must work when it is empty.
