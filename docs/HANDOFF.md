# Handoff

State of Vun as of 2026-09-28 (the nineteenth session), for the next agent. Read this, then
`CLAUDE.md` (rules and the agent procedure; on disk but ignored by the user's global gitignore, so
`jj st` never shows it), `docs/spec/00-principles.md`, `docs/spec/decisions.md` and the spec
files of the area you touch. The code is the detail; this file says where things are and why.

## 1. What this is

Vun (`.vun`, CLI `vun`) is a systems language designed by the user, compiled by a Rust
workspace. **No magic**: every primitive is a `= _` declaration of the source package `lang`,
syntax desugars only into `lang`, `std` has no privilege, nothing runs that the program did not
write. Spec in `docs/spec/`, Typst whitepaper in `whitepaper/`, Vun sources in `lib/`, test
programs in `corpus/`.

Built: lexer, postorder CST, error-recovering parser, HIR, packages (manifests, BLAKE3 Merkle
hashes, `.tar.zst`, store, MVS, `lock.vun`), a persisted parallel query engine, sema (types as
values, compile-time evaluation through MIR and an interpreter, impls, reflection, code values,
coroutines, closures, layout, kinds, narrowing), MIR with place, borrow and panic checks, LIR with
its analyses and legalization, a C99 backend and a native backend after TPDE for x86-64 and
arm64, ELF objects, the `gnu` linker adapter, the machine's toolchain file (cross compilers and
runners), styled and JSON output, explanation pages for every diagnostic. `vun run` builds and
runs programs through either backend, for the host or (through an emulator) for linux.arm64.
Linux programs are static by default, on a runtime calling the kernel (decision 279).

## 2. How the user works (non-negotiable)

- Fast iteration; requirements arrive mid-task and are integrated at once; keep going without
  stopping; commit each finished batch with `jj` (conventional one-line subjects, no AI
  mention). Surface choices that are genuinely theirs go through `AskUserQuestion` with concrete
  options and a recommendation; otherwise pick and report.
- Design: one rule per concept, no edge cases, minimal keywords, deep namespacing, lints for
  conventions, full traceability, typed codegen. Clean, elegant, performant code.
- Code: generic and data-driven (tables and handles, never names as text), single source of
  truth, enums over flags, structured errors, methods on owning types, a verifier per
  representation, files at most 300 lines with tests in `tests.rs`, clippy pedantic clean,
  `cargo nextest run`. More crates rather than bigger ones.
- Writing: no em dashes, no unicode arrows in prose. Replies are caveman-terse (a hook); code,
  docs and commits are written normally.
- Other Claude sessions may work in the same `jj` working copy at once: coordinate by message,
  claim files before rewriting them, and commit only your paths (`jj commit <paths>`).

## 3. Repository map

```
lib/lang/lang/syntax/{grammar,desugar}.vun   tokens, nodes, parse tables; appendix B as tables
lib/lang/lang/errors.vun + errors/           every diagnostic id and its explanation page
lib/lang, lib/std, lib/targets               `lang` primitives, `std`, one `lang.target.Spec` per target
corpus/{accept,reject,planned,support}       programs; `//~ id` annotations; waiting for APIs; stand-ins
compiler/<group>/<crate>                     base < lang < isa, object < front < mid < low < asm < back, link, render < pkg < tools
fuzz/                                        cargo-fuzz targets `lex`, `parse` (outside the workspace)
whitepaper/                                  Typst paper; it quotes real numbers and `vun` output
README.md                                    for people: pitch, status, how to try; contributor detail stays here
```

The whitepaper was last refreshed on 2026-09-27: its numbers (the state table and the code-size
figure of section 12, the pipeline figure), the proposed rules of section 14 and the diagnostic of
section 11 are measured from the repository and go stale; refresh them when an area lands. The
title page and the README carry a notice on AI use (the design is the user's, most code and prose
written by agents under their direction); keep it when rewriting either.

Generated, never edited (edit the `.vun` source, run `cargo gen`): `lang/grammar/src/generated/`,
`front/desugar/src/generated/`, `base/diag/src/errors/`, `front/sema/src/primitives/`,
`lang/items/src/{lang,std,targets}/` (a handle per public declaration: Rust names `lang` and
`std` only through `vun_items`).

## 4. Commands

```sh
cargo fmt && cargo clippy --all-targets && cargo nextest run && cargo gen --check && cargo xtask lines && cargo xtask layers
cargo xtask corpus            # verdicts, coverage, what stops MIR most
cargo xtask size              # the distributed binary against its budget (20 MiB)
cargo run -q -p vun-cli -- run|build [--release] [--backend b] [--target t] <file>
cargo run -q -p vun-cli -- debug tokens|cst|manifest|plan|hir|types|mir|layout|program|lir|code|frames|coverage|compare <file>
cargo run -q -p vun-cli -- debug queries <file>   # what the saved queries hold, per query and pool
cargo run -q -p vun-cli -- toolchain components
cargo run -q -p vun-cli -- debug compare --target linux.arm64 <file>   # through qemu, see below
VUN_BLESS=1 cargo nextest run -p vun-syntax    # snapshots, only after reading the diff
cd fuzz && cargo +nightly fuzz run parse -- -max_total_time=120
```

Cross builds read the machine's toolchain file (spec 12 section 6, decision 192):
`~/.config/vun/toolchain.vun` on this machine names `aarch64-linux-gnu-gcc` with Fedora's
sysroot (`/usr/aarch64-redhat-linux/sys-root/fc43`) and `qemu-aarch64 -L <sysroot>/usr` for
`linux.arm64`; without it, the backend tests skip arm64.

Logging: `VUN_LOG=<filter>` (`info` steps, `debug` units of work, `trace` elements), timings
`--timings`. Saved queries live in `.vun/queries/` beside the root file; builds in `.vun/build/`.

## 5. The compiler, by area

- **Infrastructure** (`compiler/base`): `vun-index`, `vun-intern` (process-wide `Symbol`),
  `vun-origin` (origins equal by anchored identity), `vun-diag` (`Msg`, `Diagnostic`),
  `vun-query` (slot tables, fingerprints, lock-free reads, persistence through `vun-codec`),
  `vun-par` (`par_map`), `vun-fs`, `vun-hash`, `vun-trace` (logs, timings), `vun-term` (styled
  text, tables, status lines, the shell), `vun-target` (`TargetSpec`, `Profile`, `Backends`,
  `Readiness`, `Arches`).
- **Front end** (`compiler/front`): `vun-syntax`, `vun-hir` (the front end as queries), `vun-sema`:
  `ty/` (types, values, relations, borrows table), `env/` (evaluation, calls read once by
  `Env::passing`), `typeck/` (the `Typer`, operators through interface impls, narrowing, staged
  `$for`/`$if`), `layout/` (`Shapes`), `witness/`, `check/`, `target/` (targets read from values),
  `plan.rs` (`std.build` plans).
- **Middle** (`compiler/mid`): `vun-mir` (places with projections, `build/` per form, `Concrete`
  bodies, `Programs`), `vun-mir-passes` (places, borrows, panics), `vun-interp` (runs MIR at
  compile time, loans for `&mut`).
- **Low** (`compiler/low`): `vun-lir` (SSA with block parameters, verifier, text format;
  `analysis/` `Cfg`, `Loops`, `Order`, `Liveness`, `Moves`, `StackUse`; `legalize/` wide
  integers into halves and `f16` through `f32`), `vun-lower` (MIR to LIR, frames of coroutines).
- **Backends** (`compiler/back`, spec 13 section 8): `vun-backend` (`Backend::emit(Request)`,
  `BackendInfo`, `Lack`); `vun-backend-c` (C99 as data, one printer, shards); the native backend
  as TPDE's layers: the instruction sets in the base layer, `vun-isa`, `vun-isa-x86_64` and
  `vun-isa-arm64` (every instruction once as a table of forms generated from `lang.arch`, read by
  the encoder, the printer, form selection and sema's check of assembly blocks); `vun-asm`
  (code buffers, labels, fixups, relocations, listings, the `Blocks` walk of an assembly block),
  `vun-asm-x86_64` (encoding, each form checked against yaxpeax-x86 in tests, and the calling
  conventions' registers, `conventions`), `vun-asm-arm64` (same idiom, yaxpeax-arm checks), `vun-object` (ELF objects as data), `vun-codegen` (the framework
  over `Arch` and `Isel`: assignments, registers, frames, jumps, fusion, callee-saved registers
  for loops, snippets and their encoder, `pieces`), `vun-backend-native` (a table of
  architectures, `ARCHES`, from which `INFO.arches` derives; per architecture `abi.rs`
  over the assemblers' conventions, `snippets.rs` with every operation as data, `isel/` for calls, copies,
  atomics, frames and terminators; one object writer and listing for both; tests per
  architecture, the arm64 ones decoding every word with yaxpeax-arm). Stubs: cranelift, qbe,
  llvm, external.
- **Choosing a backend** (decision 182): targets list backends per profile
  (`lang.target.Backends`); `Registry::choose` takes the first the toolchain has that is
  `Readiness::Ready` and handles the arch, `--backend` overrides, `--release` picks the release
  list and adds the C compiler's `optimize` flags; refusals are `backend.unavailable`,
  `backend.unsupported`, `backend.unchosen`. `vun toolchain components` shows each component's
  status and what it handles.
- **Linkers** (`compiler/link`): `LinkPlan`, `LinkerInfo` with readiness; `gnu`, `mold` and
  `wild` link (through the target's C compiler); `lld`, `msvc` and `external` are stubs.
- **Tools** (`compiler/tools`): `vun-driver` (`Session`, stages, codegen, builds, stacks,
  compare, the command tree, `toolchain.rs`: the chosen target equipped with the machine's tools,
  `Session::command` running through the runner), `vun-command` (declarative CLI, completions, JSON output), `vun-fetch`,
  `vun-build` (manifests, locks, the store, `toolchain`: the machine's file), `vun-harness`, `vun-generate` (`cargo gen`), `xtask`, `vun-cli`.
- **Libraries**: `lang` (primitives, operator impls over primitives, float soft routines) and
  `std` (`option`, `result`, `cmp`, `mem`, `seq`, `iter`, `sync`, `os`, `fmt`, `diag`, `task`,
  `build`, `lints`, `panic`, `testing`).

Progress: corpus 278 passed; 517 tests; the backend tests build every program in
`compiler/tools/driver/tests/driver/programs/` through C and native, for linux.x86_64 and (when
the machine runs it) linux.arm64 and wasi.wasm32, and require equal results.

## 6. Decisions

Every decision is accepted but these, proposed and waiting for the user: 249 to 251, 253 to
257, 259, 260, 262, 265, 266, 268, 269, 271, 273 (integers in patterns), 274 (`$` forms that
cannot run where written), 275 (`$if` branch scopes), 276 (C naked functions), 277 (ELF32), 278
(`layout.offset`), 280 (32-bit AAPCS in `vun-abi`). 279 (static Linux targets) is the user's choice. New language rules go
in as proposed unless the user approves them.

## 7. State and next steps

As of 2026-09-28 (nineteenth session, five agents converged): everything is committed in the main
chain, no workspace holds work but Thumb's, 517 tests pass, clippy, `cargo gen --check` and `lines` clean.
History lives in `jj log` and `decisions.md`; this section keeps only what is open.

Just landed:
- Static Linux (decision 279): `targets.linux.{x86_64,arm64}` (ABI `kernel`) run on
  `std.os.linux.kernel` (naked `_start`, system calls through one `Call` record, `string.h`
  exports in `kernel/memory.vun`, the checks' state through `mmap`); `targets.linux.gnu.*` for
  C interop. Both backends, both arches, checks included (`driver/tests/driver/runtime.rs`).
- Naked functions: native (no frame; verifier `Reason::Naked`), C (`__attribute__((naked))`,
  b4, 276); asm symbols keep their functions reachable (`InstKind::addresses`); a fixed asm input
  is read from its register; an asm statement's temp is unit.
- 2f: `std.check.State`/`mapped`, handed-out table and quarantine; typeck tables declared once;
  `vun doc` documents types as types. a6: `comptime.runtime`/`unsupported` (274), `$if` branch
  scopes (275), `layout.offset` (278), borrows of compile-time values at run time, diagnostics
  repeated per instantiation shown once. b4: patterns (273), ELF32 and Thumb relocations (277).

In flight, not in main:
- Native Thumb: the Thumb-2 table and `vun-asm-thumb` are in main (ytonorkq, yqmrxmyz; sema reads
  `Instruction.mnemonic`). Next: sema's `asm/thumb.rs` reads fp registers, memory, conditions and
  `.w`; then Thumb codegen (Isel, AAPCS rows in `compiler/isa/thumb/src/conventions.rs`,
  `ARCHES`), LIR halves to limbs by the target's widest integer, CPU features (single or double
  float), a static `linux` Thumb target under `qemu-arm`. The `thumb-82` workspace is spent.
- Milestone 3a landed (decision 280): `Rules::{AAPCS, AAPCS_VFP}` (word, stack alignment,
  `returned`, `Floats`, `Split`), backfill in `vun-abi`'s banks, `Loc::Split { parts, rest }` for
  straddling records (handled by the x86-64 and arm64 isel), wide results as `AbiType::Wide`
  (`Ret::Split` of halves), `Unpassable::Wider` + `Rules::fits` for legalization; clang checks
  `thumbv7em-none-eabihf` and `thumbv7m-none-eabi`. No `Loc::Pair`: a LIR value never spans
  registers. Open: `lang.target.Convention` gains `aapcs` (soft float) with the Thumb rows;
  legalize splits a soft `f64` by `Rules::fits`; variadic AAPCS-VFP calls use the base standard
  (`Signature` has no variadic flag yet); `Rules` declared in `lang` and generated; a target check
  that the convention's word matches `pointer_bits`. The `abi-3a` workspace is spent.
- For the user: forget and delete the `regalloc-2f` workspace (`ws-regalloc`, 3.2 GB, content
  ported); `site/` and `docs/references/` are theirs, untracked; `~/.config/vun/toolchain.vun`
  needs a `linux.gnu.arm64` row (same compiler, sysroot and runner as `linux.arm64`) for glibc
  arm64 programs; the static `linux.arm64` row still works as it is.

Next, most useful first:
1. Backends: Thumb as above; `macos.arm64` linking (lld, an SDK); riscv64 (`qemu-riscv64` is here,
   no sysroot); DWARF `DW_TAG_subprogram`s; memory regions into `LinkPlan` and a bare-metal
   runtime (`stm32f4` has an empty vector table and no `init_memory`).
2. Kernel runtime: threads, files, clocks as system calls; riscv64 numbers; `Stream` errors from
   `-errno`.
3. Checks (268): a board and a WASI shadow, use after return; `fixed` kinds.
4. Corpus gaps (`cargo xtask corpus`): `lang.simd.chunks`; lowering stops in
   `coroutines/cancel.vun` (a constant) and `integers/narrowing.vun` (a conversion).
5. Plans (`vun build build.vun`): `flash`, `test_on`, dependency parameters, lint levels.
6. `std`: `task.Pool`, `testing.Sim`, `Str` and collections, generic bodies over `$A: Allocator`.
7. Evaluator: loans inside aggregate arguments, memory per nesting level. Packaging: signed
   caches, planned builds, `vun pkg why`. A comptime-heavy benchmark before perf audits.
8. Refactors: `render/sarif`; `DiagId` and pass ids are
   strings; regalloc loads spilled operands where the form takes memory; LIR has no undefined
   value (padding copied as zeros); fuzz target `lir.rs` (written, never run).

## 8. Pitfalls

- Origins are equal by identity: lowering that makes two entities from one piece of syntax gives
  each its own origin (`Lower::own`); `Hir::verify` catches it. `OriginId` has no order.
- A query key must equal another only when computing either gives the same value: a key equal by
  origin alone kept the index a node had before an edit, so a saved memo was computed at the
  wrong node (decision 195). A key carrying anything `compute` reads must compare it.
- Query cycles (decision 198): a value computed from a `cycle` value is provisional and never
  memoized, only the query reached again keeps its memo; verification treats a memo reached
  again through its reads as `Why::Cycle`. Programs with cycles (recursive types through
  `kinds`) still re-save their queries on every warm run: the recomputed memos' reads differ in
  order from the saved ones. Cheap, but a fixpoint-style cycle (salsa's) would remove it.
- Process-wide tables (`SourceFiles::global()`, `OriginTable::global()`, interners) are shared by
  every session in the process.
- Persisted queries need unique `Query::NAME`s and codecs; set queries are `INPUT`. Delete
  `.vun/queries/` to start cold. Query values hash by content (`BTreeMap`); reports are data.
- `unset CARGO_TARGET_DIR` if the shell inherits one: builds must go to the repository's
  `target/` (a second `jj` workspace needs its own). `cargo clean` after moving the repository.
- Nothing has a target until the driver chooses one; code outside the driver sets `TargetOf`.
- `cargo gen` never depends on what it generates: change the source, run it, fix consumers.
  Generated Vun is built with `vun_syntax::make`, never formatted as text.
- `cargo fmt` rewraps code: edit scripts must assert their replacements applied.
- Checks report only what they know: unknown types and `Unevaluated` values are silence.
- Vun gotchas: `or` is a keyword; `$if` cannot bind a pattern; child namespaces are not in scope
  by bare name inside their parent; private fields are visible only inside their struct literal;
  kind attributes go on the type's literal; inside a `lang` namespace file its own names shadow
  `lang`'s; integer arithmetic is exact (`x * 2` for `u32` does not fit `u32`); no operator may
  be written inside `lang`'s operator impls; attributes are bare only from the prelude's
  namespaces.
- Native backend: an instruction's implicit registers belong in its form (`.implicit`), or a
  snippet's scratch may land in them; a terminator's operands are read after `Cx::leave`, which
  counts their uses but frees registers only in `Cx::left`. A snippet step has at most 4
  operands (`OPERANDS` in the encoder). A result is placed after its step's other operands, so
  taking a dying operand's register never unbinds one the step still reads. An instruction
  patched when the function ends is re-emitted into a listed buffer and swapped in with
  `Buffer::replace`, so the listing shows the final value (arm64's frame size; x86-64 still
  overwrites its immediate and lists the placeholder).
- arm64: homes past 256 bytes below `x29` take an index built in `x15` (`FAR`); `x16`/`x17` are
  the framework's carrier and temp, so nothing else may use them across a jump's moves.
- The toolchain file is read by every session that chooses a target, tests included; a broken
  `~/.config/vun/toolchain.vun` breaks them all. Its field errors reuse the `manifest.*` ids, as
  the lock file's do: a generic id set for literal files is a small follow-up.
- Driver integration tests are one binary (`tests/driver/main.rs`); sema unit tests run on a
  stand-in `lang` without operator impls. Snapshots are written only with `VUN_BLESS=1` (`0` or
  empty is off), and stale ones fail.
- Before committing sema or `std` changes, run `cargo nextest run -p vun-driver`: the corpus does
  not exercise `std.fmt.print`'s staged paths.
- A table in `desugar.vun` naming `lang` items is a tuple `.{ }`: its items have types of their
  own, and an array would not type. Borrow modes are lists of `BorrowWord` (`grammar.vun`).
- The parser's depth bound counts each link of a chain; past it the rest of the expression is one
  `Error` node, so later stages never recurse deeper than about twice `MAX_NESTING`.
- Saved queries are stamped with the compiler build that wrote them; another build's are refused
  and recomputed (decision 241), so a changed query type never decodes into garbage.
- Typing checks both branches of an `if` against the expected type, even when the condition is
  known: `x: Int(0..=0) = if true { 0 } else { 1 }` is `int.does_not_fit`.
- `BackendCaps` is a `Feature` set; unimplemented backends are `vun_backend::Planned`;
  `Store::stage` returns a per-process `Staging` guard removed on drop; `xtask`, `vun-command`
  and `generate` declare tool-local diagnostic ids.
- `fuzz` is a workspace member (workspace lints, one lockfile); `cargo +nightly fuzz` still
  runs from `fuzz/` and builds into the root `target/`.
