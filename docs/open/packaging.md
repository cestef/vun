# Open: packaging, releases, components

- Release `0.2.0` failed in CI (run 38382176498): `cargo gen --check` reports `editors/{tree-sitter,zed,vscode}`
  stale against `lib/lang/lang/syntax`, and the macOS arm64 package job failed in `cargo xtask pgo`
  (`tool.profile_failed`: cannot measure `build-hello`). `0.1.0` passed on a retag.
- `install.sh` trusts its first toolchain as far as TLS.
- Packaging: signed caches, planned builds, `vun pkg why`.

## Pre-evaluated output (settled direction)

No standalone "pre-compile, then compile from that" command.

- *Cache:* saved query memos already keep what compiling computed, keyed by content. Missing is a
  code cache below them; finishing it gives this for free.
- *Residual source, wanted:* a view `vun debug dump expanded` (not a `View` variant yet), a row of `vun_driver::view::View`
  (editors offer it too): the program with each `$` form evaluated, `$for` and `$if` expanded,
  generated types spelled out. A view only, never a build input: emitted text loses origins.
- *Distributable artifact, later:* prebuilt package entries in the shared store (Nix direction),
  keyed by content and target (`TargetOf`). Only instantiations the package reaches ship; generic
  code ships as HIR and types; `embed`, the build plan and the toolchain version go into the key.

## Components (498, proposed)

Built and run end to end on am3 (arm64); every ready kind is a component. Open:

- The assist of a backend that cannot express everything is the host's: the first registered
  backend with `Feature::Assist` for the target, asked through `Ask::Assist`. A component names
  what it cannot work without in `needs`; packaging refuses a need no profile holding it has.
- Opening a library costs 100 to 200 microseconds, so all are opened when a registry is made.
- Boundary cost (standard library's `hello`, native backend, am3): loaded 650 ms wall against
  585 ms linked (777 against 685 with one thread). The unit (280 KB) and its code (160 KB) are
  encoded once per call. `--timings` shows `component <kind>` and `component ask`.
