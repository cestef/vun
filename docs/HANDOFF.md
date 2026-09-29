# Handoff

State of Vun as of 2026-09-29, for the next agent: what is open and what comes next. Nothing here
is durable; conventions and how-tos live in [`dev/`](dev/workflow.md), rules in
[`spec/`](spec/00-principles.md), history in `jj log` and [`decisions.md`](spec/decisions.md).
Read `CLAUDE.md` and [`dev/workflow.md`](dev/workflow.md) first.

## State

Everything is committed in the main chain; no `jj` workspace holds work. 517 tests pass; clippy,
`cargo gen --check` and `lines` are clean; the corpus has 295 passing programs.

Built: the front end, packages and the store, the persisted parallel query engine, sema, MIR and
its checks, LIR with legalization, the C and wasm backends, the native backend for x86-64, arm64
and Thumb-2, ELF and Mach-O objects, the `gnu`, `mold` and `wild` linkers, the toolchain file,
static Linux runtimes calling the kernel (threads, storage per thread, files, clocks),
backtraces. Programs run through either backend on linux.x86_64, linux.arm64, linux.arm (through
`qemu`) and wasi.wasm32.

Last landed: `std.fmt.print` takes its arguments as a pack (315); docs split into `HANDOFF.md`
(state only) and `docs/dev/` (conventions, commands, crates, pitfalls); README rewritten with a
code card (`assets/`), MIT OR Apache-2.0 licensing, `CONTRIBUTING.md`; `design/` removed. Before
that: the `vun-sema` split into the `sema` group (`int`, `float`, `ty`, `core`, `plan`;
primitives in `vun-primitives`), Mach-O arm64 thread-local variables (314), a thread's backtrace
naming the calls that spawned it (313).

## Decisions waiting for the owner

Proposed, not yet accepted: 249 to 251, 253 to 257, 259, 260, 262, 265, 266, 268, 269, 271, 273
(integers in patterns), 274 (`$` forms that cannot run where written), 275 (`$if` branch scopes),
276 (C naked functions), 277 (ELF32), 278 (`layout.offset`), 280 (32-bit AAPCS in `vun-abi`), 281
(clocks), 283 (query soundness), 284 (files, typed errors, builds checking library bodies). 279
(static Linux targets) and 282 (`&mut T` where `&T` is) were the owner's choice.

## Known gaps in landed work

- **Unsound memo, in main since storage per thread (312 to 314):** `VUN_QUERY_CHECK=1 cargo nextest
  run -p vun-driver` fails most backend tests: the `mir` memo of a function of
  `std/os/linux/kernel/tls.vun` computes a different value under an equal fingerprint (even with
  `target/tmp/memos` cleared). The plain suite passes. Fix at the source before more sema or MIR work.
- A `$`-generic C import (a pack inferred per call) is typed nowhere yet: such calls stay
  silently untyped (decisions 285, 286).
- Thumb: 9 of the 13 driver programs agree with x86-64 (last count).
- ABI: legalize splits a soft `f64` by `Rules::fits`; `Rules` should be declared in `lang` and
  generated; no target check yet that the convention's word matches `pointer_bits`.
- Backtraces: no call line per frame (native marks each call's origin; C cannot say where a
  call returns); no macOS (compact unwind, PAC). WASI reports the site only.
- Mach-O arm64 thread-locals are checked by linking with `ld64.lld`, never run on a Mac.
- A new project still writes a whole new shared memo file (~450 ms dev, after the build): memos
  whose keys name the project's functions but read shared inputs alone count as shared.
- Truly cold (empty store), threads wait ~1.1 s on the same `std` claims, once per toolchain.

## For the owner

- Forget and delete the `regalloc-2f` workspace directory (`ws-regalloc`, 3.2 GB, content ported)
  if it is still on disk.
- `~/.config/vun/toolchain.vun` needs a `linux.gnu.arm64` row (same compiler, sysroot and runner
  as `linux.arm64`) for glibc arm64 programs.

## Next steps, most useful first

1. Backends: finish Thumb (the remaining driver programs); `macos.arm64` linking (lld, an SDK);
   riscv64 (`qemu-riscv64` is here, no sysroot); DWARF `DW_TAG_subprogram`s; memory regions
   into `LinkPlan` and a bare-metal runtime (`stm32f4` has an empty vector table and no
   `init_memory`).
2. Kernel runtime: riscv64 numbers; directories, seeking, environment.
3. Checks (268): a board and a WASI shadow, use after return; `fixed` kinds.
4. Corpus gaps (`cargo xtask corpus`): `lang.simd.chunks`; lowering stops in
   `coroutines/cancel.vun` (a constant) and `integers/narrowing.vun` (a conversion).
5. Plans (`vun build build.vun`): `flash`, `test_on`, dependency parameters, lint levels.
6. `std`: `task.Pool`, `testing.Sim`, `Str` and collections, generic bodies over `$A: Allocator`.
7. Evaluator: loans inside aggregate arguments, memory per nesting level. Packaging: signed
   caches, planned builds, `vun pkg why`. A comptime-heavy benchmark before perf audits.
8. Refactors: `vun-sema` itself is one dependency cycle (`env`, `eval`, `typeck`, `decl`,
   `member`, `layout`, `target`, `check`, `witness`, `asm`, about 12k lines): evaluation needs
   typed bodies and typing needs evaluation. Splitting further means owning traits in the lower
   part, as `Evaluator` does (`typeck` could reach `asm` through a registered checker). Also:
   `render/sarif`; `DiagId` and pass ids are strings; regalloc loads spilled operands where the
   form takes memory; LIR has no undefined value (padding copied as zeros); fuzz target `lir.rs`
   (written, never run).
