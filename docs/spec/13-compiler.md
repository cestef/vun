# 13. Compiler architecture

Written in Rust as a set of libraries with narrow interfaces. Every stage after the frontend can be
replaced: backends, linkers, targets, passes, evaluators, diagnostic renderers. Nothing above LIR
knows about any backend; nothing above the linker adapter knows about any linker.

## 1. Pipeline

```
 source
   |  vun-syntax      tokens + postorder CST arrays; error-recovering parser
   v
 HIR                   desugaring into `lang` items (appendix B), name resolution
   |  vun-sema        demand-driven, parallel: type checking and compile-time evaluation interleaved
   v
 MIR                   control-flow graph over places; one body per instantiation
   |  MIR passes        checks: place states, kinds, borrows, scoping, panic.never, stack
   |                    transforms: coroutines, defer, range-based check elision, instrumentation
   +--> vun-interp    reference semantics: runs compile-time code and tests
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
  `(kind, anchor, subtree_size)` in the style of Carbon; lossless tree views are built lazily for
  the formatter and the LSP only. Every token anchors exactly one node, so the tree plus the trivia
  table is the source; a node whose token is absent (a missing `;`, a missing operand) is anchored
  where it was expected and was reported by the parser. The verifier checks that subtree sizes
  nest, leaves have no children, the root is `File`, every token has exactly one owner, and every
  subtree covers a contiguous run of tokens.
- HIR, MIR and LIR are struct-of-arrays arenas addressed by `u32` handles. Zig measured 20% to 40%
  wall-time gains from this layout alone.
- Types, compile-time values and names live in one sharded intern pool. A symbol is equal by
  pointer, hashes by the content hash of its text and orders by its text, so query traces and
  output built from symbols do not depend on the process or the schedule.

**Queries.**
- The unit of incremental work is a declaration or a function body, never an expression.
- Every query result has an interface hash; dependents are re-run only when the interface changed
  (early cutoff, as Go does per package). A memo keeps a verifying trace (Mokhov, Mitchell and
  Peyton Jones, *Build systems à la carte*): the interface hash of each query it read. In a later
  revision it is reused when every dependency, brought up to date, hashes as recorded, and it is
  checked at most once per revision, as Salsa's memos are. Traces hold content hashes rather than
  revision numbers, so they stay valid in the next process. Inputs are set, and a set that leaves
  an input's interface unchanged starts no revision. A query that reaches itself gets the value
  its `cycle` rule gives, with the path of queries, instead of looping.
- The query database is persisted to disk from phase 1, with a bounded cache for bodies (new Salsa
  initially grew rust-analyzer's memory about 4x). A query opts in when its keys and values are
  data (`Persist`, encoded by `vun-codec`, which has no dependencies; an interned value
  persists by content and is interned again when read); its computed memos are saved with their traces, by query name,
  in `.vun/queries/` under the toolchain's hash. A memo whose trace reads a query that is not
  persisted is not saved, and a loaded memo is verified before its first use like any memo of an
  earlier revision, so a stale file costs recomputation, never a wrong result.

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
  edges before checking). A `yield` has two successors: where resuming goes on, and a cancel path
  running what is pending there, which ending the suspended frame takes (terminator `Ended`).
- **Programs.** A program is what its root file's exports reach (spec 08, section 1): from each
  `#[link.export]` function, every function a body calls, with the `$` arguments of the call, or
  takes as a value, and every declaration the linker provides (query `Programs`). Code
  generation reads each reached instantiation *concrete* (query `Concrete`): its compile-time
  values evaluated into constants that flow into the operands reading them, and every local
  typed. A function that is not concrete stops the program's code generation with the reason
  (`vun dump program` lists them).
- **Layouts.** Every type with a run-time layout has a shape (query `Shapes`): its size and
  alignment, the offset of each part, how an enum tells its variants (an index, or a niche
  pattern), and its unused patterns (spec 05, section 6). `vun dump layout` prints them.
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

The grammar (tokens, node kinds, their fields) is defined once, in `lang/syntax/grammar.vun`, as Vun
data. From it are generated: the Rust kind enums and typed accessors used by the parser and every
compiler crate; `lang.syntax`, which compile-time code uses to read code values (spec 03); the
formatter's node table; and the tree-sitter grammar for editors. Tokens and nodes are therefore
defined in exactly one place and cannot drift.

`cargo gen` reads the grammar with the compiler's own parser as literal data (the
reader of `build.vun` manifests, spec 12) and translates each declaration by the shape of its
value: a `PascalCase` record of lists is a kind, an `enum` an enum, any other list a table whose
element type is inferred; a table whose rows are records of several fields is a list of a row
struct named after it (`iteration` holds `IterationRow`s), read by field name, and a column of
paths into `lang` (`"lang.ops.Try.branch"`) is generated as their handles, members of type
constructors and interface requirements included. Rust reads what a handle declares through it
(`Item::parent`, `Item::symbol`), and a borrow mode is a row of the table `borrows`
(`vun_desugar::Borrow`), never its words. `grammar.vun` goes to `vun-grammar`, which the parser is built
from, and `desugar.vun` to `vun-desugar`, which only later stages read. The same task generates the
diagnostic ids of `vun-diag` from `lang/errors.vun`, a typed handle (`vun_items::Item`) for every
public declaration and namespace of `lang` and `std` (`vun_items::lang::int::unsigned`), through
which Rust code names a declaration, never by a string, and the compiler's primitives from `lang`
itself: every top-level declaration
of a `lang` namespace whose value is `_` is one, and each namespace becomes an enum of its
primitives (`primitives::mem::Mem`), wrapped by `Primitive`, so the compiler matches each
namespace's primitives apart and a primitive it does not implement fails to compile. A hole
inside an interface's requirement struct is a requirement, not a primitive. A test fails when a
generated file is stale.

The generator is its own binary, `vun-generate` (`cargo gen`), built from only what reading `.vun`
data needs (syntax, grammar, diagnostics): no crate it depends on reads a generated table or
handle, and what it depends on that is generated (token and node kinds, diagnostic ids) refers to
nothing else, so a generated change breaking any consumer never keeps it from running again
(decision 132).

**Node fields.** A branch kind declares its children as tagged slots, in source order and without
punctuation: `.binding(.optional(.\mut), .one(.pattern), .one(.value))`. A slot is `one`,
`optional` or `many`; a field named after a node kind holds a node of that kind, any other field
any node. The generator derives a field enum and each kind's slots; `Cst::fields` assigns a node's
children to them (left to right, backtracking, leaving a node to a later slot restricted to its
kind), and the CST verifier requires every node to fit. Each distinct shape is its own kind, so a
kind's fields never depend on its anchor token: `x := e` is `binding` and `x: T = e` is
`typed_binding`, `.{ }` is `struct_lit` and `T{ }` is `typed_lit`. Malformed source is an `error`
node, which holds what recovery skipped and fits any slot. Dumps print each child's field.

## 6. Compile-time evaluation

`vun-interp` interprets MIR under an explicit target model (layout, endianness, ranges, traps,
pointer provenance). It is the definition of the language's semantics, and it is also what runs
compile-time code, tests in the default mode, and doctests.

Evaluation strategy (research notes, section 1):
1. each MIR body is lowered on first use to a compact register bytecode, cached by the query engine;
2. memory follows the target byte model; values are interned only when they escape into a type or
   constant;
3. compile-time calls are memoized on (function, interned arguments) and persisted across builds;
4. the evaluator runs on its own stack with a deterministic step budget;
5. a Cranelift JIT may later sit behind `Evaluator`, differential-tested against the interpreter.

Sema computes the values that need no function body itself: literals, names and members of
namespaces, `struct` and `enum` literals (nominal, decision 28), ranges, tuples, function types, and
primitive type constructors applied to constants (`Int(0..=255)`); a call goes to the evaluator.
Types come from the same values: a parameter or local has its annotation's type, a namespace
member its annotation or its value's type, and each function body is typed on its own query. A
compile-time call is an instance: its body, and what it builds, are evaluated with its parameters
bound to its arguments, so the fields of `Pair(u8)` have type `u8`, and a `struct` or `enum`
literal evaluated in it is identified by the literal and the instance (decision 28). A
value or type that is not known yet is unknown, and checks that would need it stay silent rather
than guess.

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

- **Targets are data written in Vun.** A `TargetSpec` (word size, endianness, alignments, ABIs,
  features, object format, entry symbol, default backend and linker) is a compile-time value from
  the `targets` package, a `lang.target.Spec`: architectures, object formats, backends and linkers
  are `lang.meta.Name`s, sets the registries extend, and endianness is `lang.target.Endian`. Adding a target adds a file; the compiler is not rebuilt.
- **One binary, three kinds of component** (decision 131). `vun` is one executable, and what it
  holds is decided per component:
  - *compiled in*: the front end, checks, MIR, the interpreter, `fmt`, the language server,
    packages, diagnostic renderers and the pure-Rust backends (C text, Cranelift); enough to check,
    build and run a program with nothing else installed. Cargo features choose the set.
  - *a native library loaded on first use*: libclang for `vun tool bind`, libLLVM for the LLVM
    backend, through a thin adapter crate each, the only crates allowed `unsafe`. Never linked in;
    missing, it is one diagnostic naming the library and the versions accepted, and every other
    command works.
  - *a process*: third-party backends and linkers speak LIR text and `LinkPlan` JSON. Rust has no
    stable dylib ABI, and processes isolate failures. Never the toolchain's own features, whose
    input would then be another program's output.
- **The binary has a size budget.** `cargo xtask size` builds the `dist` profile and fails past the
  budget, a constant of the task (`BUDGET`, bytes of the stripped binary on the host), set from
  the first measured build with the libclang adapter (6,382,768 bytes, rounded up to 64 KiB);
  raising it, like a new external crate, is a decision stating the cost.
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
  base/    index  span  intern  origin  diag  query  pass  tool  target  trace  items  json
  front/   grammar  desugar  syntax  hir  sema
  mid/     mir  passes  interp
  low/     lir  abi  passes
  back/    api  c  cranelift  qbe  llvm  process
  link/    api  wild  mold  lld  gnu  msvc  process
  render/  short  annotate  ariadne  json  sarif
  tools/   build  fmt  bind  clang  probe  harness  driver  lsp  cli  generate  xtask
lib/
  lang/     primitives (declarations with `= _`), syntax/grammar.vun
  std/      prelude, iter, task, mem, sync, fmt, text, meta, diag, lints, log, hw, os, simd, testing, build,
            collections
  targets/  one namespace per architecture: targets.<arch>.<system>
  boards/   example runtimes: stm32f4, rp2040, esp32c3
```

Directory names are short because the path gives context; packages are named `vun-<name>`
(`compiler/back/c` is `vun-backend-c`, `compiler/back/api` is `vun-backend`).

Rules:
- `vun-driver` is a library; the CLI, the LSP and the test harness are thin shells over it.
- A crate depends only on crates in earlier groups: `base` < `front` < `mid` < `low` <
  `back`, `link`, `render` < `tools`. Where a later stage implements something an earlier one
  needs, the earlier one owns the trait (`vun-sema` defines `Evaluator`, `vun-interp`
  implements it).
- Components declare their facts once (`BackendInfo`, `LinkerInfo`); support checks derive from
  them. Tables declared once generate their enums and lookups (tokens, diagnostic ids, commands).
- Every arena is an `IndexVec` addressed by its own index newtype (`vun-index`).
- Logging goes through `vun-trace` (`VUN_LOG`, `VUN_LOG_FORMAT`, `VUN_TIMINGS`); corpus
  and snapshot tests go through `vun-harness` (`VUN_BLESS`).
- There is one frontend: the LSP runs on the same query engine as the compiler.
- Command lines are declared, never matched by hand: `vun-command` holds command trees (names,
  typed parameters, subcommands, actions) as data, and parses, renders help and reports usage
  errors as diagnostics, with suggestions, from them. Options declared on a group apply to every
  command below it (`vun --format short dump cst f.vun` and `vun dump cst f.vun --format short`
  are one request), and a program's `configure` step applies them before the command runs. `vun-driver` declares the `vun` tree;
  `xtask` declares its own with the same crate.
