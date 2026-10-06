# Handoff

The session-to-session note: what is in flight and what to do next. Everything else that is open
lives by area in [`open/`](open/); history is in `jj log` and [`decisions.md`](spec/decisions.md),
conventions in [`dev/`](dev/workflow.md), rules in [`spec/`](spec/00-principles.md).

Keep this file under 40 lines. When an item here is done, delete it; when it is not for the next
session, move it to the file of its area.

## State

Everything is committed; tests, clippy, `lines` and `layers` are clean. `cargo xtask corpus` shows
what the corpus waits for. Decisions awaiting the owner are the rows marked `proposed` in
`decisions.md`.

## Next

The owner's queue, in order:

1. Try the server in a real editor, which polish waits for ([`open/editors.md`](open/editors.md)).
2. Backends: the rp2350 on hardware, wasm constants bound narrower than their type
   ([`open/backends.md`](open/backends.md)).

## For the owner

`~/.config/vun/toolchain.vun` needs a `linux.gnu.arm64` row (same compiler, sysroot and runner as
`linux.arm64`) for glibc arm64 programs.

## Areas

| File | Holds |
|------|-------|
| [`open/editors.md`](open/editors.md) | editor packages, the language server |
| [`open/backends.md`](open/backends.md) | wasm, targets, runtimes, boards (rp2350) |
| [`open/language.md`](open/language.md) | typing gaps, evaluator, `vun-sema` refactor |
| [`open/std.md`](open/std.md) | the library and the corpus |
| [`open/testing.md`](open/testing.md) | properties, fuzzing, simulation, benchmarks |
| [`open/performance.md`](open/performance.md) | compile speed |
| [`open/packaging.md`](open/packaging.md) | releases, the store, components |
| [`open/site.md`](open/site.md) | website, playground, brand |
