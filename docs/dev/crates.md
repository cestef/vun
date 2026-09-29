# The compiler, by area

Where the code for each part lives. The workspace layout and its layering rules are in
[spec 13 section 9](../spec/13-compiler.md); the design of each stage is in the rest of spec 13.

- **Infrastructure** (`compiler/base`): `vun-index`, `vun-intern` (process-wide `Symbol`),
  `vun-origin` (origins equal by anchored identity), `vun-diag` (`Msg`, `Diagnostic`),
  `vun-query` (slot tables, fingerprints, lock-free reads, persistence through `vun-codec`),
  `vun-par` (`par_map` on a process pool, the one `unsafe` module), `vun-fs`, `vun-hash`,
  `vun-trace` (logs, timings), `vun-term` (styled text, tables, status lines, the shell),
  `vun-target` (`TargetSpec`, `Profile`, `Backends`, `Readiness`, `Arches`), `vun-harness`
  (corpus, snapshots, `//~` annotations, `llvm` checks of assembled code), `vun-html`,
  `vun-data` (structured values apart from any format, `ToData`/`FromData` derived by
  `vun-data-derive`, written as JSON, TOML or Lua tables, JSON read), `vun-tree-sitter`
  (tree-sitter grammars and queries as typed values, `grammar.js` and `.scm` written, the trees
  `tree-sitter parse` prints read back), `vun-uri` (RFC
  3986 references: typed components, resolution, normalization, a builder, file URIs), the
  derives' shared `vun-derive-core` (items read from tokens, tokens built tree by tree) and
  `vun-variants` (`#[derive(Variants)]`: an enum's `ALL`).
- **Rust views of `lib/lang`** (`compiler/lang`, generated): `vun-grammar` (tokens, nodes, parse
  tables), `vun-items` (a handle per public declaration), `vun-primitives`.
- **Instruction sets** (`compiler/isa`): `vun-isa` (`Isa`, `Blocks`, `Width`, immediates,
  `Processor`), `vun-inline` (assembly blocks as data), `vun-isa-{x86_64,arm64,thumb}` (every
  instruction once as a table of forms generated from `lang.arch`, read by the encoder, the
  printer, form selection and sema's check of assembly blocks; calling conventions' registers,
  `conventions`).
- **Objects** (`compiler/object`): `vun-object` (objects as data, written as ELF through
  `vun-elf` or as Mach-O), `vun-elf` (ELF files as data, written and read, each record declared
  once per class), `vun-dwarf`.
- **Front end** (`compiler/front`): `vun-syntax` (with `literal`: `.vun` data read without
  evaluation, and `FromLiteral`, derived by `vun-literal-derive`, reading it as typed values),
  `vun-desugar`, `vun-hir` (the front end as queries).
- **Sema** (`compiler/sema`): `vun-sema-int` (exact integers, intervals), `vun-sema-float`
  (compile-time IEEE floats), `vun-sema-ty` (types, values, relations, borrows table, item keys,
  `call` values: `Func`, `Instance`, `Instantiation`; `trap`: traps and `Unevaluated`),
  `vun-sema` (the query cycle: `env/` evaluation, calls read once by `Env::passing`; `typeck/` the
  `Typer`, operators through interface impls, narrowing, staged `$for`/`$if`; `layout/`
  (`Shapes`), `witness/`, `check/`, `target/` (targets read from values), `asm/`, `apply/`),
  `vun-sema-plan` (`std.build` plans). Methods of `vun-sema` on the lower crates' types are
  extension traits (`Instantiate`, `Parameters`, `Attributed`, `Apply`, ...).
- **Middle** (`compiler/mid`): `vun-mir` (places with projections, `build/` per form, `Concrete`
  bodies, `Programs`), `vun-mir-passes` (places, borrows, panics), `vun-interp` (runs MIR at
  compile time, loans for `&mut`), `vun-sanitize` (run-time checks of unsafe code).
- **Low** (`compiler/low`): `vun-lir` (SSA with block parameters, verifier, text format, constant
  folding of one operation; `analysis/` `Cfg`, `Loops`, `Order`, `Liveness`, `Moves`,
  `StackUse`; fixtures behind the `testing` feature), `vun-lir-passes` (the transforms as
  extension traits: `Simplify` and its rounds, `Legalize::apply` for wide integers into limbs,
  soft floats and division), `vun-lower` (MIR to LIR per function, frames of coroutines),
  `vun-abi` (calling conventions as `Rules`: SysV, AAPCS64, AAPCS and AAPCS-VFP).
- **Assemblers** (`compiler/asm`): `vun-asm` (code buffers, labels, fixups, relocations,
  listings, the `Blocks` walk of an assembly block), `vun-asm-x86_64` (each form checked against
  yaxpeax-x86 in tests), `vun-asm-arm64` (yaxpeax-arm checks), `vun-asm-thumb` (checked against
  llvm-mc).
- **Backends** (`compiler/back`, spec 13 section 8): `vun-backend` (`Backend::emit(Request)`,
  `BackendInfo`, `Lack`, `Traced`); `vun-backend-c` (C99 as data, one printer, shards);
  `vun-backend-wasm`; `vun-codegen` (the framework over `Arch` and `Isel`: assignments,
  registers, frames, jumps, fusion, callee-saved registers for loops, snippets and their encoder,
  `pieces`); `vun-regalloc`. Stubs: cranelift, qbe, llvm, external.
- **Native backend** (`compiler/native`, TPDE's layers): `vun-native` (the `Native` trait, whose
  `compile` and `object` every architecture shares; `select` with `Carries`, `Frames`,
  `Terminates`; the object writer, listing, inline assembly and widths; fixtures behind
  `testing`), `vun-native-{x86_64,arm64,thumb}` (per architecture `abi.rs`, `snippets.rs` with
  every operation as data, `isel/`; the arm64 tests decode every word with yaxpeax-arm),
  `vun-backend-native` (the table of architectures, `ARCHES`, from which `INFO.arches` derives,
  and the object formats).
- **Choosing a backend** (decision 182): targets list backends per profile
  (`lang.target.Backends`); `Registry::choose` takes the first the toolchain has that is
  `Readiness::Ready` and handles the arch, `--backend` overrides, `--release` picks the release
  list and adds the C compiler's `optimize` flags; refusals are `backend.unavailable`,
  `backend.unsupported`, `backend.unchosen`.
- **Linkers** (`compiler/link`): `LinkPlan`, `LinkerInfo` with readiness; `gnu`, `mold` and
  `wild` link (through the target's C compiler); `lld`, `msvc` and `external` are stubs.
- **Renderers** (`compiler/render`): diagnostics as short lines, annotated snippets, JSON; SARIF
  is a stub.
- **Packages** (`compiler/pkg`): `vun-build` (manifests, locks, the store, `toolchain`: the
  machine's file), `vun-fetch` (archives, mirrors), `vun-http` (the `https` scheme).
- **Tools** (`compiler/tools`): `vun-driver` (a library: `Session`, stages, packages loaded into
  the session, codegen, builds, stacks, compare, `toolchain.rs`: the chosen target equipped with
  the machine's tools, `Session::command` running through the runner), `vun-cli` (the `vun`
  command tree, a shell over the driver), `vun-command` (declarative CLI, completions, JSON
  output), `vun-doc`, `vun-lsp` (the server: a table of methods, each a protocol type the server
  `Answers` or `Hears`), `vun-lsp-protocol` (the protocol's values and methods as types),
  `vun-generate` (`cargo gen`), `xtask` (`check`, `hooks`,
  `corpus`, `lines`, `layers`, `size`, `typeset`).
- **Libraries**: `lang` (primitives, operator impls over primitives, float soft routines,
  `lang.mem.soft`) and `std` (`option`, `result`, `cmp`, `mem`, `seq`, `iter`, `sync`, `os`,
  `fmt`, `diag`, `task`, `time`, `trace`, `build`, `lints`, `panic`, `testing`).

## Tests

Driver integration tests are one binary (`compiler/tools/driver/tests/driver/main.rs`). The
backend tests build every program in `compiler/tools/driver/tests/driver/programs/` through C
and native, for linux.x86_64 and, when the machine runs them, linux.arm64, linux.arm and
wasi.wasm32, and require equal results. A test budgets every function of a hosted program at
2000 instructions. Sema unit tests run on a stand-in `lang` without operator impls.
