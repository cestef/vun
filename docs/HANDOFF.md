# Handoff

What is in flight and what to do next. The rest that is open is by area in [`open/`](open/).
History is in `jj log` and [`decisions/`](spec/decisions/README.md), conventions in
[`dev/`](dev/workflow.md), rules in [`spec/`](spec/00-principles.md). Under 40 lines.

## State

Reviewed 2026-11-24: all proposed decisions are `accepted` but 658 (the progress bars, waiting on
its uncommitted change in `compiler/base/term/src/progress/`); `cargo xtask decisions` checks them.
Clippy, the suite (1109 tests) and every xtask check were clean on am3.
`0.2.0` is tagged, its release CI failed ([`open/packaging.md`](open/packaging.md)), the causes are fixed.
The site is rebuilt on Astro, Starlight and Solid 2; watch its first CI `pnpm test` ([`open/site.md`](open/site.md)).

## Next, in order
1. Retag `0.2.0` (editors regenerated, lld installed for the macOS `pgo` job) and watch the CI run.
2. Check the cost of the stale-memo fix: `Groups::regroup` now counts own inputs
   ([`open/performance.md`](open/performance.md)); then drop `Session::untargeted` if it holds.
3. Rerun the HIR `lower` fuzz target and the others on am3 (a stray attribute crash is fixed).
4. The conventions adopted from the sibling projects, what is left: [`open/testing.md`](open/testing.md).
5. Priming (664 to 668): `cargo xtask package`, a second layer, other targets. Fuzzing: rerun
   `vun debug smith` on am3 (`--jobs 3 --chunk 5 --warm`), triage.

The owner's queue: the server in a real editor ([`open/editors.md`](open/editors.md)); the rp2350
on hardware, the UART byte arriving as `0xFC` ([`open/backends.md`](open/backends.md)); the
`vun component` commands against a signed release (659 to 661).

## Areas

| File | Holds |
|------|-------|
| [`open/editors.md`](open/editors.md) | editor packages, the language server |
| [`open/backends.md`](open/backends.md) | wasm, targets, runtimes, boards (rp2350) |
| [`open/language.md`](open/language.md) | typing gaps, evaluator, `vun-sema` refactor |
| [`open/std.md`](open/std.md) | the library and the corpus |
| [`open/ffi.md`](open/ffi.md) | C interop: what the binder, the linker and the runtimes lack |
| [`open/testing.md`](open/testing.md) | properties, fuzzing, simulation, benchmarks |
| [`open/performance.md`](open/performance.md) | compile speed |
| [`open/packaging.md`](open/packaging.md) | releases, the store, components |
| [`open/site.md`](open/site.md) | website, playground, brand |
