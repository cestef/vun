# 13. Compiler architecture

Written in Rust as a set of libraries with narrow interfaces. Every stage after the frontend can be
replaced: backends, linkers, targets, passes, evaluators, diagnostic renderers. Nothing above LIR
knows about any backend; nothing above the linker adapter knows about any linker.

## 1. Pipeline

```
 source
   |  overt-syntax      tokens + postorder CST arrays; error-recovering parser
   v
 HIR                   desugaring into `lang` items (appendix B), name resolution
   |  overt-sema        demand-driven, parallel: type checking and compile-time evaluation interleaved
   v
 MIR                   control-flow graph over places; one body per instantiation
   |  MIR passes        checks: place states, kinds, borrows, scoping, panic.never, stack
   |                    transforms: coroutines, defer, range-based check elision, instrumentation
   +--> overt-interp    reference semantics: runs compile-time code and tests
   v
 LIR                   type-erased SSA: i8..i128, f16..f64, ptr, vectors; loads, stores, calls
   |  LIR passes         ABI lowering per backend capabilities, identical-body merging
   v
 Backend (trait)       C | QBE | Cranelift | LLVM | external process
   v
 Artifacts             objects, assembly, C source, in-memory code
   |  Tool steps        assembler, C compiler, when a backend emits text
   v
 Linker (trait)        wild | mold | lld | GNU ld | MSVC link | none
```

## 2. Speed is an architectural property

Research notes, section 3: in every compiler measured, the front end dominates once the backend is
fast (TPDE: backend is 15% of clang -O0), and retrofitting parallelism onto a query system takes
years (rustc's parallel front end). So both are designed in from phase 1.

**Data layout.**
- Tokens are one array with a side table for trivia. The CST is a postorder array of
  `(kind, token, subtree_size)` in the style of Carbon; lossless tree views are built lazily for the
  formatter and the LSP only.
- HIR, MIR and LIR are struct-of-arrays arenas addressed by `u32` handles. Zig measured 20% to 40%
  wall-time gains from this layout alone.
- Types, compile-time values and names live in one sharded intern pool.

**Queries.**
- The unit of incremental work is a declaration or a function body, never an expression.
- Every query result has an interface hash; dependents are re-run only when the interface changed
  (early cutoff, as Go does per package).
- The query database is persisted to disk from phase 1, with a bounded cache for bodies (new Salsa
  initially grew rust-analyzer's memory about 4x).

**Parallelism.**
- Sema claims declarations on worker threads. Code generation is per function, with one thread
  emitting and linking (Zig's split).
- Output is independent of scheduling: diagnostics are sorted, symbols are ordered by stable keys,
  and CI runs the compiler with a randomized scheduler to prove it.

**Whole program.** Packages are source-only, so every build sees all LIR. Code generation for
LLVM and C is sharded by function hash to stay parallel.

**Deduplication.** LIR is type-erased, so a structural hash of each lowered body merges
`List(u32)` and `List(i32)` exactly. This is the sharing rustc could not do at MIR level. Linker
identical-code folding runs on top.

**Measured from the start.** Synthetic corpus generators produce large programs; CI tracks lines
per second per stage and fails on regressions.

## 3. The two IRs

- **MIR** keeps what the checks need: places, borrow modes, enums, checked operations, suspension
  points, origins. Place-state and borrow checking are dataflow problems on its CFG, so they are
  non-lexical and treat every kind of control flow the same way (`defer` and `yield` become explicit
  edges before checking).
- **LIR** is what every backend has in common: SSA values of scalar and vector types, memory
  operations, calls, branches. Range types, enums and aggregates are lowered before it. A new backend
  translates LIR and nothing else.
- Both have **stable text formats**. Tests are written against them, dumps are readable and carry
  origins, and an external backend is any executable reading LIR text on standard input.

## 4. Provenance invariant

Every token, CST node, HIR node, MIR statement, LIR instruction, artifact symbol and link-plan entry
carries an `OriginId` (spec 10, section 1). Passes that create entities must attach an origin with
the pass recorded in `Via::Lower`. IR verifiers, which run between passes in debug builds of the
compiler, reject any entity without one.

## 5. One grammar, many consumers

The grammar (tokens, node kinds, their fields) is defined once, in `lang/syntax/grammar.ov`, as Overt
data. From it are generated: the Rust kind enums and typed accessors used by the parser and every
compiler crate; `lang.syntax`, which compile-time code uses to read code values (spec 03); the
formatter's node table; and the tree-sitter grammar for editors. Tokens and nodes are therefore
defined in exactly one place and cannot drift.

## 6. Compile-time evaluation

`overt-interp` interprets MIR under an explicit target model (layout, endianness, ranges, traps,
pointer provenance). It is the definition of the language's semantics, and it is also what runs
compile-time code, tests in the default mode, and doctests.

Evaluation strategy (research notes, section 1):
1. each MIR body is lowered on first use to a compact register bytecode, cached by the query engine;
2. memory follows the target byte model; values are interned only when they escape into a type or
   constant;
3. compile-time calls are memoized on (function, interned arguments) and persisted across builds;
4. the evaluator runs on its own stack with a deterministic step budget;
5. a Cranelift JIT may later sit behind `Evaluator`, differential-tested against the interpreter.

Every backend is checked against the interpreter by differential testing, on the corpus and on
randomly generated programs in the style of Csmith. That is what keeps guarantees 3 and 4 true.

## 7. Extension points

```rust
/// Code generation. Backends declare capabilities; the driver adapts LIR to them.
pub trait Backend {
    fn info(&self) -> &BackendInfo;   // id, supported arches, capabilities; support derives from it
    fn emit(&self, unit: &Unit, target: &TargetSpec, out: &mut dyn ArtifactSink, diag: &mut DiagCtxt)
        -> Result<(), ErrorGuaranteed>;
}

/// Linking. Receives a target-independent plan: objects, sections, regions, symbols, entry.
pub trait Linker {
    fn info(&self) -> &LinkerInfo;    // id, object formats, whether regions can be placed
    fn link(&self, plan: &LinkPlan, target: &TargetSpec, diag: &mut DiagCtxt) -> Result<(), ErrorGuaranteed>;
}

/// External tools: assemblers and C compilers for text-emitting backends and build steps.
pub trait Tool {
    fn id(&self) -> &str;
    fn run(&self, inv: &Invocation) -> std::io::Result<Output>;
}

/// Analyses and transforms over MIR or LIR, with declared dependencies.
pub trait Pass<Ir> {
    fn id(&self) -> &str;
    fn requires(&self) -> &[&str];
    fn run(&self, body: &mut Ir, cx: &PassCx) -> PassResult;
}

/// Compile-time and test evaluation.
pub trait Evaluator {
    fn call(&mut self, f: FnRef, args: &[Value]) -> Result<Value, Trap>;
}

/// Diagnostic output: terminal (annotate-snippets or ariadne), short, JSON, SARIF, LSP.
pub trait DiagSink {
    fn emit(&mut self, d: &Diagnostic, origins: &OriginTable, sources: &dyn SourceMap);
}
```

- **Targets are data written in Overt.** A `TargetSpec` (word size, endianness, alignments, ABIs,
  features, object format, entry symbol, default backend and linker) is a compile-time value from
  the `targets` package. Adding a target adds a file; the compiler is not rebuilt.
- **Static registration by default** (Cargo features choose what is compiled in), **dynamic
  registration by process**: external backends and linkers speak LIR text and `LinkPlan` JSON. Rust
  has no stable dylib ABI, and processes isolate failures.
- **The link plan is abstract.** Memory regions and sections from the build go into `LinkPlan`; the
  lld and GNU ld adapters turn them into linker scripts, others into flags.
- **Capabilities are checked, never faked.** A program needing guaranteed tail calls, frame sizes or
  vectors on a backend that lacks them is rejected with a diagnostic naming the backends that can.

## 8. Backends and linkers

| Backend | Role | Targets | Notes |
|---------|------|---------|-------|
| C | reach | anything with a C99 compiler | first; vendor toolchains, Cortex-M, AVR, Xtensa; defined C only, `-fwrapv -fno-strict-aliasing`; `restrict` from aliasing facts; frame sizes via `-fstack-usage` |
| Cranelift | default debug backend | x86_64, aarch64, riscv64, s390x | in process; roughly 20% less codegen time than LLVM for rustc, code about 14% slower |
| QBE | small and simple | amd64, arm64, rv64 | subprocess on IL text; no vectors (lowered), no tail calls, no 32-bit targets |
| LLVM | optimized builds, widest native reach | ARM32/Thumb, RV32, AVR, wasm, ... | optional; `noalias`, `musttail`, stack-size sections |
| baseline (later) | fastest debug builds | x86_64, aarch64 | a single-pass LIR backend in the style of TPDE (8x to 24x faster than LLVM -O0) |

| Linker | Use |
|--------|-----|
| wild | development links on Linux ELF; aiming at incremental linking; check linker-script support before firmware use |
| mold | fast, mature ELF |
| lld | ELF, COFF, Mach-O, wasm, linker scripts; default for embedded |
| GNU ld | vendor toolchain compatibility |
| none | objects or static libraries for foreign builds |

## 9. Workspace

```
compiler/
  base/    index  span  intern  origin  diag  query  pass  tool  target  trace
  front/   grammar  syntax  hir  sema
  mid/     mir  passes  interp
  low/     lir  abi  passes
  back/    api  c  cranelift  qbe  llvm  process
  link/    api  wild  mold  lld  gnu  msvc  process
  render/  short  annotate  ariadne  json  sarif
  tools/   build  fmt  bind  probe  harness  driver  lsp  cli  xtask
lib/
  lang/     primitives (declarations with `= _`), syntax/grammar.ov
  std/      prelude, iter, task, mem, sync, fmt, meta, diag, lints, log, hw, os, simd, testing, build, collections
  targets/  one namespace per architecture: targets.<arch>.<system>
  boards/   example runtimes: stm32f4, rp2040, esp32c3
```

Directory names are short because the path gives context; packages are named `overt-<name>`
(`compiler/back/c` is `overt-backend-c`, `compiler/back/api` is `overt-backend`).

Rules:
- `overt-driver` is a library; the CLI, the LSP and the test harness are thin shells over it.
- A crate depends only on crates in earlier groups: `base` < `front` < `mid` < `low` <
  `back`, `link`, `render` < `tools`. Where a later stage implements something an earlier one
  needs, the earlier one owns the trait (`overt-sema` defines `Evaluator`, `overt-interp`
  implements it).
- Components declare their facts once (`BackendInfo`, `LinkerInfo`); support checks derive from
  them. Tables declared once generate their enums and lookups (tokens, diagnostic ids, commands).
- Every arena is an `IndexVec` addressed by its own index newtype (`overt-index`).
- Logging goes through `overt-trace` (`OVERT_LOG`, `OVERT_LOG_FORMAT`, `OVERT_TIMINGS`); corpus
  and snapshot tests go through `overt-harness` (`OVERT_BLESS`).
- There is one frontend: the LSP runs on the same query engine as the compiler.
