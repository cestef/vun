# Handoff

The session-to-session note: what is in flight and what to do next. Everything else that is open
lives by area in [`open/`](open/); history is in `jj log` and [`decisions.md`](spec/decisions.md),
conventions in [`dev/`](dev/workflow.md), rules in [`spec/`](spec/00-principles.md). Under 40 lines: delete done items, move later ones to
their area's file.

## State

Everything is committed; tests, clippy, `lines` and `layers` are clean. `cargo xtask corpus` shows
what the corpus waits for. Decisions awaiting the owner are the rows marked `proposed` in
`decisions.md`.

## Next

Fuzzing (`vun debug smith`, spec 11, [`open/testing.md`](open/testing.md)): a 2000-program campaign
(seeds 30000.., run reach) found two bugs, both fixed (an unreached block read a value `fold`
dropped; arm64 `f16` truncation to `i32` past the format's range). Next: rerun it on am3 (`scripts/remote
--host am3.wg`, `--jobs 3 --chunk 5`, `--warm` once after a rebuild), triage what it finds.

CLI rework (decisions 659 to 661, proposed) is tried end to end: `vun component list|add|remove`
against a signed release on am3 (`VUN_RELEASES=file://<dir>` laid out as a forge's). A package is
shown by its directory in `Building <dir>`.

The owner's queue:
1. Try the server in a real editor, which polish waits for ([`open/editors.md`](open/editors.md)).
2. Backends: the rp2350 on hardware ([`open/backends.md`](open/backends.md)).

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
