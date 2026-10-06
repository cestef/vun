# Open: website and brand

`site/` (Vite, TypeScript): landing page, playground (`/play/`), course (`/learn/`, lessons in
`site/lessons/`, `pnpm check:lessons`). `.github/workflows/site.yml` deploys to GitHub Pages when
`main` changes it.

- A host must send the headers of `site/public/_headers`, or the editor has no language server.
- A program that never ends runs until stopped; threads are not offered (WASI has none).
- Brand kits (`dev/assets.md`): removing outputs of variants no longer declared, as `cargo gen`
  does; code in the tall and square clips is small at 80 columns; the OBS overlays are untried in
  OBS.
