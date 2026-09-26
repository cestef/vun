# Handoff

State of Vun as of 2026-09-26 (end of the fifteenth session), for the next agent. Read this, then
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
compiler/<stage>/<crate>                     base < front < mid < low < back, link, render < tools
fuzz/                                        cargo-fuzz targets `lex`, `parse` (outside the workspace)
whitepaper/                                  Typst paper; it quotes real numbers and `vun` output
```

Generated, never edited (edit the `.vun` source, run `cargo gen`): `front/grammar/src/generated/`,
`front/desugar/src/generated/`, `base/diag/src/errors/`, `front/sema/src/primitives/`,
`base/items/src/{lang,std,targets}/` (a handle per public declaration: Rust names `lang` and
`std` only through `vun_items`).

## 4. Commands

```sh
cargo fmt && cargo clippy --all-targets && cargo nextest run && cargo gen --check && cargo xtask lines
cargo xtask corpus            # verdicts, coverage, what stops MIR most
cargo xtask size              # the distributed binary against its budget (12 MiB)
cargo run -q -p vun-cli -- run|build [--release] [--backend b] [--target t] <file>
cargo run -q -p vun-cli -- debug tokens|cst|hir|mir|layout|program|lir|code|frames|compare <file>
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
  as TPDE's layers: `vun-asm` (code buffers, labels, fixups, relocations, listings),
  `vun-asm-x86_64` (every instruction once as a table of forms read by the encoder, the printer
  and form selection, each form checked against yaxpeax-x86 in tests), `vun-asm-arm64` (same
  idiom, yaxpeax-arm checks), `vun-object` (ELF objects as data), `vun-codegen` (the framework
  over `Arch` and `Isel`: assignments, registers, frames, jumps, fusion, callee-saved registers
  for loops, snippets and their encoder, `pieces`), `vun-backend-native` (a table of
  architectures, `ARCHES`, from which `INFO.arches` derives; per architecture `abi.rs`
  (System V, AAPCS64), `snippets.rs` with every operation as data, `isel/` for calls, copies,
  atomics, frames and terminators; one object writer and listing for both; tests per
  architecture, the arm64 ones decoding every word with yaxpeax-arm). Stubs: cranelift, qbe,
  llvm, external.
- **Choosing a backend** (decision 182): targets list backends per profile
  (`lang.target.Backends`); `Registry::choose` takes the first the toolchain has that is
  `Readiness::Ready` and handles the arch, `--backend` overrides, `--release` picks the release
  list and adds the C compiler's `optimize` flags; refusals are `backend.unavailable`,
  `backend.unsupported`, `backend.unchosen`. `vun toolchain components` shows each component's
  status and what it handles.
- **Linkers** (`compiler/link`): `LinkPlan`, `LinkerInfo` with readiness; only `gnu` links
  (through the target's C compiler).
- **Tools** (`compiler/tools`): `vun-driver` (`Session`, stages, codegen, builds, stacks,
  compare, the command tree, `toolchain.rs`: the chosen target equipped with the machine's tools,
  `Session::command` running through the runner), `vun-command` (declarative CLI, completions, JSON output), `vun-pkg`,
  `vun-build` (manifests, locks, the store, `toolchain`: the machine's file), `vun-harness`, `vun-generate` (`cargo gen`), `xtask`, `vun-cli`.
- **Libraries**: `lang` (primitives, operator impls over primitives, float soft routines) and
  `std` (`option`, `result`, `cmp`, `mem`, `seq`, `iter`, `sync`, `os`, `fmt`, `diag`, `task`,
  `build`, `lints`, `panic`, `testing`).

Progress: corpus 213 passed, 10 planned, 31 shown, 0 failed; 326 tests; the backend tests build
every program in `compiler/tools/driver/tests/driver/programs/` through C and native, for
x86-64 and (when the machine runs it) linux.arm64, and require equal results on all four.

## 6. Decisions

Every decision in `decisions.md` is accepted, 1 to 193 (the user asked this session that all that
make sense be accepted; 138, 157, 161 and 166 are amended by 182 and 183). New language rules go
in as proposed unless the user approves them.

## 7. Next steps, in order

1. Backends (the user's focus): (a) arm64: done for linux.arm64 in the sixteenth session
   (decision 192); next `macos.arm64` (Mach-O in `vun-object`, Apple's AAPCS64 variant: narrow
   arguments extended by the caller, stack arguments packed, `x18` reserved), then riscv64 the
   same way (an assembler after `vun-asm-arm64`, `Arch`, `Isel`). This machine has
   `qemu-riscv64` but no riscv64 linker or sysroot (Fedora packages none; `lld` or
   `gcc-riscv64-linux-gnu` need the user to install them), and nothing to link or run Mach-O.
   (b) Native code quality, seventeenth session (decisions 200 to 203): slots promoted to SSA
   values after lowering (`Unit::promote`), constants folded by the builder, branches falling
   into the next block, no self moves, `mov r/m8, imm8`, parameters and call arguments moved
   straight into their registers: 13 to 49 percent fewer instructions on the backend test
   programs. Left: values live across blocks are still homed at every block end (a
   callee-saved register only for loops), aggregates copied between slots (SROA would split
   them), `if c { 1 } else { 0 }` diamonds from MIR, a snippet's operand moved into a fixed
   register it takes. (c) DWARF line tables and symbol names done (decisions 204, 205:
   `vun-dwarf`; `gdb` breaks on Vun lines); the unit is named by the first file with a row;
   next unwind information (`.eh_frame`) and `DW_TAG_subprogram`s; (d) data objects aligned past every C type in the C backend, memory regions
   into `LinkPlan`, a bare-metal runtime, a slice value (`xs[a..b]` not borrowed) reported by
   sema rather than as `backend.unlowered`; (e) linkers other than `gnu`.
2. Performance and persistence. Done in the sixteenth session (decision 191): saved queries
   written whole past twice their compacted size, atomic `Edit::put`, reads saved without their
   key's fingerprint (11.4 to 6.8 MB on ten programs sharing a directory), queries decoded in
   parallel, `vun debug queries`; mimalloc (cold compiles 19% faster); `vun_hash::Stable`
   instead of BLAKE3 for symbols, interned values and origins, token and item lookups by table
   (10% more); `FileRef.path` a `Symbol`; the query engine's code shared across queries (the
   binary 470 KB smaller). `par_map`'s spawns measured: about 1 ms a run, not worth a pool.
   The size budget is 12 MiB (decision 193, the user's choice): the dist binary is 11.7 MiB
   at the end of the seventeenth session (316 KiB left);
   the largest parts are `std` (1.7 MiB of text), rustls and ring (640 KiB, for `vun-http`) and
   per-query code (`Cx::get`, `Table::slot`, about 400 KiB). (b) Done in the
   seventeenth session (decision 197): saved queries load as used (`persist/store.rs`: a
   query's keys when one is looked for, by `Stable` hash; a memo when its key gets a slot, placed
   in the entry before it is published, so no `arc_swap` store; reads resolved on first verify);
   a save extending the file keeps every key's position and copies what was never asked for.
   Load went from 7.5 to 0.4 ms; a warm check sharing a file with ten programs 15.1 to 11.1 ms
   (45 to 11 ms of CPU). What is left of a warm run is reading packages (`stage syntax`, about
   4 ms: 168 files read and hashed) and process start. Decision 198 fixed cycles found on the
   way (see pitfalls). Left: (c) The audit items with no workload to
   measure yet: fingerprints of large values, the interpreter copying whole values, small
   `Integer`s; the corpus has no program past 70 ms, so a comptime-heavy benchmark comes first.
3. `std`: `task.Pool`, `testing.Sim`, a Linux runtime without libc, `Str` and collections,
   `print`'s `.fixes`; generic bodies over a constrained `$A: Allocator`.
4. Checks: leaks at scope exits inside a body, `fixed` and `scoped` kinds, frames' `end` running
   their defers; borrows of top-level bindings as free values, frames' regions, a label at the
   later conflicting use.
5. `vun build` carries out a plan's actions (`std.build.Action` by handles), with a program's
   options as a typed record.
6. MIR and sema gaps (`cargo xtask corpus` lists them): `.{ }` and `.name` in plain
   instantiations, `lang.simd.chunks`, `#[layout.offset]`, traps in a callee's instantiated signature; `&.word(3)` (a
   borrow of a compile-time enum value) at run time stops at `backend.unlowered`. Or-patterns
   that bind (decision 194), exhaustiveness (decision 196, `typeck/exhaust/`) and impl member
   types (decision 199; a requirement type naming another requirement is not compared yet) are
   done. Every
   typer diagnostic in a generic body is reported once per instantiation plus once plain
   (`int.does_not_fit` three times for two instantiations): dedup them by origin in `Checked`.
7. Evaluator: its own stack; loans inside aggregate arguments. Packaging: signed caches, planned
   builds, `vun pkg why`, `.workspace` visibility.

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
  stand-in `lang` without operator impls. `VUN_BLESS` blesses whenever set.
