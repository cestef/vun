# Open: website and brand

`site/` (Astro, Starlight, Solid 2): landing page, guide (`/docs/`) with its course
(`/docs/learn/`), playground (`/play/`); `pnpm test` checks the guide and the lessons
(`dev/commands.md`). `.github/workflows/site.yml` tests, builds and deploys to GitHub Pages when
`main` changes it.

- `pnpm test` has not run against the real compiler since the rework (only a placeholder
  `vun.wasm` locally): the first CI run, or a remote build, is its first check.
- The landing sting was reworked (per-property motion, one clock, fade-in drop) and checked only
  in a throttled background window: watch it once in a foreground browser.
- The lessons' prose is still thin in places; each should say what it teaches before the task.

- A host must send the headers of `site/public/_headers`, or the editor has no language server.
- A program that never ends runs until stopped; threads are not offered (WASI has none).
- Brand kits (`dev/assets.md`): removing outputs of variants no longer declared, as `cargo gen`
  does; code in the tall and square clips is small at 80 columns; the OBS overlays are untried in
  OBS.
