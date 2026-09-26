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
 Backend (trait)       native | C | Cranelift | QBE | LLVM | external
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
  pointer, hashes by the stable hash of its text and orders by its text, so query traces and
  output built from symbols do not depend on the process or the schedule. The stable hash
  (`vun_hash::Stable`, a fixed-seed foldhash) is what symbols, interned values and origins hash
  by: the same in every process of one toolchain build, and far cheaper than BLAKE3 on the few
  bytes each holds; what is saved by it is named by the toolchain that wrote it.

**Queries.**
- The unit of incremental work is a declaration or a function body, never an expression.
- Every query result has an interface fingerprint (128 bits, two fixed-seed foldhash passes,
  deterministic within a toolchain build); dependents are re-run only when the interface changed
  (early cutoff, as Go does per package). A memo keeps a verifying trace (Mokhov, Mitchell and
  Peyton Jones, *Build systems à la carte*): the fingerprint of each memo it read, once each, in
  the order first read. In a later revision it is reused when every dependency, brought up to
  date, has the fingerprint recorded, and it is checked at most once per revision, as Salsa's
  memos are. Traces hold fingerprints rather than revision numbers, so they stay valid in the
  next process. Each key gets a slot in its query's table when first asked for; reads are
  recorded, verified and compared by table and slot. Threads share a database without locks on
  the way to a memo, one computes each memo while the others needing it wait, and a thread that
  would wait on a thread waiting for it computes the memo itself. Sessions compiling for one
  target share a database, so each library is checked once per process. Inputs are set, and a set that leaves
  an input's interface unchanged starts no revision. A query that reaches itself gets the value
  its `cycle` rule gives, with the path of queries, instead of looping.
- The query database is persisted to disk from phase 1, with a bounded cache for bodies (new Salsa
  initially grew rust-analyzer's memory about 4x). A query persists its memos with their values
  (`Db::persist`) or by their traces alone (`Db::trace`); keys and values are encoded by
  `vun-codec` (`#[derive(Encode, Decode)]`). A handle into a process table (a symbol, an
  interned value, a file, an origin) is written once per file by content and recorded again when
  read, and those tables are the process's, as interners are. Memos are saved by query name and
  key position in `.vun/queries/<toolchain>` beside the root file, the toolchain named by its
  executable. A memo whose trace reads a query that is not persisted is not saved; inputs
  (`Query::INPUT`) never are. A loaded memo is verified before its first use like any memo of an
  earlier revision, so a stale file costs recomputation, never a wrong result; a traced memo
  that holds stands for its value, which is computed only if read (red-green, as rustc's
  dependency graph). Loading reads what is used, in place in the file: a query's keys when one
  of them is first looked for, found by the stable hash of the key's value (`vun_hash::Stable`,
  which symbols, interned values and origins hash by), a key's memo when that key is first asked
  for, a memo's reads when it is first verified and its value when first needed; so a run pays
  for the memos it uses, not for those of other programs sharing the file. Saving happens only
  when a memo was computed that the file did not hold, and extends the loaded stream: every key
  keeps its position, and keys and memos never asked for are copied, not decoded again. Since an
  extended stream keeps every entry of the one it extends, the file records its body's size when
  last written whole, and a body grown past twice that size is written whole on the next save,
  every saved memo read to be written anew. Files are written beside their place and renamed
  over it, so a reader or a crash sees the old file or the new, never a part. Each key is saved
  with the fingerprint its memo had when the key was first saved, and a memo or a read records
  its own only when it differs, since reads outnumber memos ten to one: a memo copied unread
  keeps meaning what it read. `vun debug queries <file>` counts what the file beside a root file holds, per query
  and per pool. A key naming a HIR node is equal to another only when both the node's place and
  its origin are: a memo is computed from its key, and a key equal by origin alone would keep the
  place a node had before an edit moved it.

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
- **Calls** pass their arguments as one reading of the callee's parameters says (`Env::passing`),
  shared by compile-time evaluation and MIR: the receiver, `$` arguments inferred by typing, code
  for a `Code(T)` parameter, a compile-time value for another `$` parameter, evaluated as the
  parameter's type by the call running the body (it may be a literal, whose identity is that
  call), and run-time values. Every body, interpreted or compiled, runs in the environment of its
  call with the types typing found. A call passing code **expands** at the call site (spec 03,
  section 4): the callee's body, instantiated with the code, is built into the caller's, and a
  `$c` whose code reads what only the caller holds (MIR `Splice`) is built there, where the code
  is written; a body with a splice left is not concrete. `_ = e` evaluates `e` into a temporary
  nothing consumes, so a linear value discarded leaks.
- **Trap sites** are calls of `lang.panic.trap` with the site's id; `lang.panic.fail(reason)` is
  one whose message table entry says its reason, and at compile time a trap site traps.
- **Programs.** A program is what its root file's exports reach (spec 08, section 1): from each
  `#[link.export]` function, every function a body calls, with the `$` arguments of the call, or
  takes as a value, and every declaration the linker provides (query `Programs`). Code
  generation reads each reached instantiation *concrete* (query `Concrete`): its compile-time
  values evaluated into constants that flow into the operands reading them, and every local
  typed. A function that is not concrete stops the program's code generation with the reason
  (`vun debug program` lists them).
- **Layouts.** Every type with a run-time layout has a shape (query `Shapes`): its size and
  alignment, the offset of each part, how an enum tells its variants (an index, or a niche
  pattern), and its unused patterns (spec 05, section 6). `vun debug layout` prints them.
- **LIR** is what every backend has in common: SSA values of scalar and vector types, memory
  operations, calls, branches. Range types, enums and aggregates are lowered before it. A new backend
  translates LIR and nothing else. Values are scalars (`i8` to `i128`, `f16` to `f64`, `ptr`) in
  blocks with parameters; aggregates live in stack slots and data objects, reached by address.
  Functions, data and imports are named by id, and only exports and imports carry link symbols.
  Under Vun's convention a value in memory is passed by address and a result in memory written
  through an address passed after the parameters; under the C convention (`#[abi.c]`) such values
  are records passed by value, their scalars given for the backend's classification. A trap site
  calls `lang.panic.trap` with a `lang.panic.Trap` whose `site` indexes the unit's message table.
- **Lowering** (`vun-lower`, query `Lowered`, `vun debug lir`) turns a program's concrete MIR into
  LIR: every local a stack slot of its layout, a top-level binding one data object, statements the
  instructions computing into their place; integers in their range's storage, converted where a
  value moves between ranges; enums by their shapes; checks of indices and narrowings as trap
  sites. A primitive no instruction computes is marked `#[lang.lower.by(f)]`: making its caller
  concrete rewrites each call of it into a call of `f`, a Vun function of the same parameters
  (`lang.float.rem` runs as `lang.float.soft.rem`), compiled like any other.
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
    fn info(&self) -> &BackendInfo;   // id, supported arches, capabilities, readiness
    /// `Request`: the unit legalized for it, the target, where origins are in source, the
    /// profile, and whether to add a listing of the code (`Emits::Listing`).
    fn emit(&self, request: &Request, out: &mut dyn ArtifactSink, diag: &mut DiagCtxt)
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
  features, object format, entry symbol, backends by profile and linker) is a compile-time value from
  the `targets` package, a `lang.target.Spec`: architectures, object formats, backends and linkers
  are `lang.meta.Name`s, sets the registries extend, and endianness is `lang.target.Endian`. Targets
  are named by system, then CPU: `targets.linux.x86_64`, `targets.bare.cortex_m4f`; the
  architecture, CPU, system and ABI are fields, never parts of a name. A `Spec` also holds the
  target's C model (`lang.target.C`: the width and alignment of each C integer type), which the C
  backend declares its types from. Adding a target adds a declaration; the compiler is not
  rebuilt.
- **The target is chosen once, and nothing assumes one.** `--target` names a declaration of
  `targets` by its path (`--target bare.cortex_m4f`); without it, `targets.hosts` gives the
  target of the machine running the toolchain, named as Rust's `std::env::consts` names it. The
  choice is the query `Chosen`, and its result the input `TargetOf`, which has no default: every
  width, byte order and layout is read from it, and is unknown until it is set.
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
  budget, a constant of the task (`BUDGET`, bytes of the stripped binary on the host), 12 MiB,
  raised from the first measured build (6,382,768 bytes) once the C and native backends took it
  to 7,658,304, and from 10 MiB once the native backend in TPDE's layers took it to 11,172,560;
  raising it, like a new external crate, is a decision stating the cost.
- **The link plan is abstract.** Memory regions and sections from the build go into `LinkPlan`; the
  lld and GNU ld adapters turn them into linker scripts, others into flags.
- **Capabilities are checked, never faked.** A program needing guaranteed tail calls, frame sizes or
  vectors on a backend that lacks them is rejected with a diagnostic naming the backends that can.

## 8. Backends and linkers

| Backend | Role | Targets | Notes |
|---------|------|---------|-------|
| C | reach | anything with a C99 compiler | first; vendor toolchains, Cortex-M, AVR, Xtensa; strict C99, defined C only (section 8.2); `restrict` from aliasing facts; frame sizes via `-fstack-usage` |
| Cranelift | default debug backend | x86_64, arm64, riscv64, s390x | in process; roughly 20% less codegen time than LLVM for rustc, code about 14% slower |
| QBE | small and simple | amd64, arm64, rv64 | subprocess on IL text; no vectors (lowered), no tail calls, no 32-bit targets |
| LLVM | optimized builds, widest native reach | ARM32/Thumb, RV32, AVR, wasm, ... | optional; `noalias`, `musttail`, stack-size sections |
| native | fastest debug builds | x86_64, arm64 | the toolchain's own: a single-pass LIR backend in the style of TPDE (8x to 24x faster than LLVM -O0), section 8.3 |
| external | anything | any | a process reading LIR text |

**Choosing a backend.** A target lists the backends it builds with under each profile, most
preferred first (`lang.target.Backends`: `debug`, what `vun build` uses, and `release`, what
`vun build --release` uses): `linux.x86_64` debug builds prefer `native`, then `cranelift`, then
`c`, and release builds `llvm`, then `c`. A build takes the first that the toolchain has (its
Cargo feature built in), that generates code (`BackendInfo.readiness`) and that generates it for
the target's architecture (`BackendInfo.arches`); that is `Registry::choose`, its reasons typed
(`Unfit::{Absent, Planned, Arch}`). `--backend` (or `VUN_BACKEND`) names one instead, which must
fit; otherwise the build is refused with `backend.unavailable` or `backend.unsupported`, naming the
backends that could, and a target none of whose preferences fit reports `backend.unchosen` with
each one's reason. The profile also reaches the tools: a release build adds the target C
compiler's `optimize` flags. `vun toolchain components` lists each backend with whether it
generates code and for which architectures. Linkers are named by the target (`linker`).

### 8.1 What every backend shares

TPDE (Schwarz, Kamm and Engelke, 2025) compiles an SSA IR in two passes per function, an analysis
and a single code generation pass, through an adapter over the IR it is given instead of a
translation into its own. LIR is designed to be that IR, so no backend translates it into another
representation before emitting:

- **Dense ids.** Values, blocks, instructions and slots are `IndexVec` indices, so every per-value
  or per-block table of a backend is an array, never a hash map.
- **One edge per pair.** A terminator jumps to a block at most once with any given arguments: two
  targets naming the same block carry the same arguments (the verifier checks it). Moves on an edge
  therefore belong to the edge's pair of blocks, as TPDE's phi moves do.
- **Semantics as data.** Each operation (`BinOp`, `Cond`, `CastOp`) declares its facts once: the
  signedness it reads its operands with and its domain, the operands on which its result is
  defined. Lowering guarantees the domain (a shift amount below the width, a divisor that is not
  0, no signed division of the minimum by -1: arithmetic is exact, spec 04), and backends rely on
  it, so no backend emits a check the language does not have.
- **Analyses** (`vun_lir::analysis`), computed once per function and read by every backend:
  - `Loops`: the loop forest, irreducible loops included (Wei, Mao, Zou and Chen, 2007), the whole
    function being the root loop;
  - `Order`: the order blocks are emitted in, reverse postorder with each loop's blocks kept
    together, which shortens the live ranges of values ending at a loop's exit;
  - `Liveness`: for each value its uses and the first and last block of its live range in `Order`,
    and whether it lives to that block's end (a value used across a back edge lives through the
    whole loop), in time linear in the instructions (Kohn, Leis and Neumann, 2018);
  - `Moves`: the parallel copy of a jump's arguments into its target's parameters, sequenced with
    one temporary per cycle.

### 8.2 The C backend

The C backend writes a translation unit in strict C99 and never relies on behavior C leaves
undefined. It builds C as data (`vun_backend_c::ast`) and prints it once; `vun debug code --backend
c` shows it, and `vun build` and `vun run` compile and link it.

- **No header, no assumed type.** The file declares its own integers, `u8` to `u64` and `i8` to
  `i64`, from the target's C model (`lang.target.C`: the width and alignment of `short`, `int`,
  `long` and `long long`, and of an address), and checks while compiling everything it relies on
  of them: that `char` is 8 bits, and the size and alignment of each type and of an address, by
  C99's negative-array idiom on `sizeof` (a `struct` of a byte then the type is its alignment plus
  its size). With no header, the only names C reserves are its keywords.
- **Integers** are `uN`: unsigned arithmetic wraps, as LIR's does. An operation reading signed
  operands (`Reads::Signed`: `SDiv`, `AShr`, `SLt`, ...) converts them to `iN`. An operand
  narrower than the target's `int` is converted to `unsigned` first, so promotion never turns
  unsigned arithmetic into signed overflow. Constants carry the suffix of the C type their width
  is (`5u`, `5ul`, `5ull`).
- **Memory** is reached by typed access, `*(u32 *)p`, through `u8 *` addresses; LIR's accesses
  are aligned by their layouts, and the C compiler's flags include `-fno-strict-aliasing`, since
  LIR memory has no types. `Volatile` access goes through a `volatile` pointer. Atomic access uses
  what the target's C compiler offers beyond C99, as its profile declares
  (`lang.target.Atomics`): the `__atomic_*` builtins of GCC and Clang, or nothing, and then it is
  refused. A link symbol spelling a builtin the file uses is refused too.
- **Stack slots** are arrays of the integer whose alignment is the slot's. **Data objects** are a
  union of a `struct` of their bytes, as runs of `u8` between the addresses their relocations write
  (address constants, `(u8 *)&d5 + 16`), and of the integer giving their alignment. An address in
  data sits at an offset the target aligns addresses to, so C places every field where the bytes
  say, and each `struct`'s size is checked. Objects are defined so that what one points to comes
  first; objects pointing to each other in a cycle are declared before, as C99 allows (6.9.2).
- **Functions.** Vun's convention passes integers as `uN`, values in memory as `u8 *`, and a
  result in memory through an address after the parameters. The C convention passes each integer
  as `iN` or `uN` as its LIR extension says (`AbiType::Int(_, Ext)`, from its range), and records
  as `struct`s of their fields, their size checked, copied between the parameter and the memory
  the body reads. Every function is declared before any is defined, `static` unless exported.
- **Control flow.** Blocks are labels, emitted in `Order` so that a jump to the next block is no
  `goto`; block parameters are locals assigned by `Moves` before the jump; `Branch` is an `if`,
  `Switch` a `switch`, `Unreachable` an empty infinite loop. A value used once, in its own block,
  with nothing writing memory or calling between its definition and its use, is written inline as
  an expression, so compare and branch read `if (v3 < v4)`, as TPDE fuses them.
- **Names.** Link symbols are written as they are and must be C identifiers that are not keywords
  (a diagnostic otherwise). The backend's own names are typed (`Own::Value(3)`), spelled in one
  place (`v3`), and one a link symbol already spells gets underscores appended until it is free.
- **Origins** become `#line` directives, so the C compiler's diagnostics and debug information
  point at Vun source.
- **What C99 lacks is refused, never approximated**: 128-bit integers, `f16`, guaranteed tail
  calls, naked functions, link sections, and atomics where the compiler has no builtins
  (`backend.lacks`).

- **Aliasing.** An address a signature passes says what else reaches what it reaches
  (`AbiType::Address(Alias)`): an exclusive borrow is `Alias::Exclusive` (spec 05, section 8), and
  C declares it `restrict`.
- **Shards.** A program of more functions than a shard holds (`SHARD`, 256) spans a header every
  file includes (types, checks, declarations), a file of its data, and shards of functions, each
  function in the shard its content hash gives, so the files compile in parallel and a changed
  function recompiles one shard. Its functions and data are then seen by the linker, under names
  tagged by the unit's content hash (`p1f3a_f3`), never another program's.

The C compiler is target data (`lang.target.CCompiler`): its command, the flags the rules above
need (`-std=c99 -ffreestanding -fno-strict-aliasing`: the output is freestanding C, declaring all
it uses, so no function name means anything to the compiler), the flags its links take (bare
metal: `-nostdlib -nostartfiles`), its atomics, and how it reports frame sizes
(`lang.target.FrameSizes`). `vun build` writes the backend's artifacts under
`.vun/build/<content hash>/` next to the root file, compiles each source with it through
`vun-tool`, in parallel, with `-fstack-usage` where it has it (`vun debug frames` prints the sizes
read back, `Backend::frame_sizes`), and links with the target's linker. The `gnu` adapter drives
GNU ld through the same compiler; a plan's memory regions and section placement become a linker
script, built as data (`Script`) and printed once. Tool arguments are typed (`vun_tool::Arg`:
text or a path), and `vun run` passes the program's exit status on.

### 8.3 The native backend

The toolchain's own backend for debug builds, in TPDE's design, reading LIR through the shared
analyses. It is layered as TPDE is, one crate per layer, so that a new architecture adds an
assembler and its instruction compilers and nothing else:

- **`vun-asm`**: what every assembler shares: a code buffer (`Buffer`), labels and the references
  to them patched when the function ends (each architecture says how, `Isa::patch`),
  relocations, the arithmetic of immediates, and listings.
- **`vun-asm-x86_64`**, **`vun-asm-arm64`**: each architecture's instructions, declared once as a
  table of forms: the kinds of operands a form takes (a register of some width, memory, an
  immediate of some width, a condition, a label), the widths it is defined at, and its encoding.
  The encoder emits an instruction by the first form its operands fit, the printer writes the
  same form's text, and callers ask which forms fit before choosing operands. So an instruction's
  encoding and its text cannot disagree, and a test decodes every form with an independent
  decoder (yaxpeax) and compares both.
- **`vun-object`**: relocatable objects as data, written once (ELF first); relocation kinds are
  the framework's, each machine numbers them.
- **`vun-codegen`**: the framework, which knows no architecture: value assignments, register
  allocation, the frame, homes, values recomputed where used, callee-saved registers for loops,
  jumps filling their targets' parameters, compare and branch fusion, and the snippet encoder.
  An architecture gives it its registers and its own moves (`Arch`) and its instruction compilers
  (`Isel`).
- **`vun-backend-native`**: per architecture, the calling conventions, the snippets and the
  instruction compilers; the unit assembled as one object; listings.

The framework:

- **One pass per function.** Blocks in `Order`, instructions in program order; instruction
  selection, register allocation and encoding happen together into a code buffer that later
  steps only patch (jumps to later blocks, the frame size).
- **Values.** Each value has an assignment, indexed by its id: its register, if any, its home in
  the frame, whether the home holds it, and its remaining uses from `Liveness`. A use decrements
  the count; at zero the register is free once the instruction is done. Constants and addresses
  known while compiling (a slot's, a datum's, a function's, at constant offsets) are recomputed
  where used, never kept: `Recompute`, found once per function.
- **Registers.** The lowest free register is taken; when none is, one is evicted round-robin,
  stored home if still used there only. Registers an instruction uses are locked until it is
  done. Values live across the blocks of a loop, deepest loops and most uses first, each hold a
  callee-saved register from definition to last use. Only the state at the current point is
  tracked: before a call and at a jump to a block entered from elsewhere every value still used
  is stored home, so each block starts with every value in its home or its own register, but a
  block emitted next that only the block before reaches, which keeps the registers.
- **Instruction compilers** ask the framework for operands (`Cx::reg`), results (`Cx::def`), a
  result overwriting a dying operand (`Cx::reuse`: no copy where the operand is used no more),
  scratch registers, and fixed registers freed (`Cx::take`).
- **Snippets** (TPDE, section 4): most operations are instruction sequences as data, over slots
  for the operation's operands (`In`, and memory at an operand's address, `At`), its result
  (`Out`), scratch registers (`Temp`), registers an instruction requires (`Fixed`, freed before
  the snippet starts) and operands taken as an immediate where a form takes one, else through the
  register the instruction requires (`Through`: a shift's count, in `cl` unless constant). The
  snippet encoder places each operand as the forms allow: an operand in a register as it is, a
  constant as an immediate, a value in its home as memory, a known address folded into a memory
  operand, else brought into a register; and a move of a dying operand into the result is no
  instruction, the result taking its register. Where TPDE derives snippets from C through LLVM,
  they are written against the instruction table: `add` is a move of the first operand into the
  result then `add` of the second, and the encoder turns that into `add rdi, 0x1` where the first
  operand dies in `rdi` and the second is 1.
- **Fusion** looks only forward within a block: a compare whose one use is the branch ending it
  is made by the branch, which jumps on the flags.
- **Frames.** Always a frame pointer; the prologue reserves the frame with the widest immediate,
  its size written when the function ends, and the callee-saved registers taken are saved below
  the frame pointer. The frame size is known exactly, so the backend reports frame sizes
  (`stack.max`, spec 07).
- **Semantics once.** Instruction sequences come from the operation facts of section 8.1 and the
  snippets; an operation an architecture cannot do in a few instructions (128-bit division,
  `f16`) calls a helper written in Vun and compiled through the same pipeline.
- **Output.** Functions are compiled in parallel into their own buffers; one thread assembles the
  object, with relocations for calls, data and imports. `vun debug code --backend native` prints
  each function's listing: its instructions with their offsets and bytes, under the line of
  source each LIR instruction comes from.

As built (x86-64 System V, ELF): values are held in the registers a call may overwrite but
`r10` and `r11`, which the framework keeps for its own moves, and those of loops in `rbx` and
`r12` to `r15`. Vun's convention passes the signature's values in the registers C passes
integers in, then on the stack; the C convention classifies records as System V does (integer
records of up to 16 bytes in registers, larger ones in memory, a result in memory through an
address passed first). Narrow integers are held with undefined upper bits and extended where an
operation reads them (division, right shifts, conversions, C arguments). A float is held as its
bits in a general register, like an integer, so moves, spills and selects need nothing of their
own; arithmetic, comparisons and conversions move the bits through `xmm0` and `xmm1`.
Conventions pass floats in SSE registers, and a small record's eightbytes in general or SSE
registers by their class (System V: SSE when every field in it is a float); a C call sets `al`
to the SSE registers used. LIR's float comparisons are ordered (false where either operand is a
NaN); lowering gives IEEE `!=` as `==` negated, and follows each float operation with a select of
the canonical NaN where its result is unequal to itself. `rem` and `mul_add` run as
`lang.float.soft`'s functions (`#[lower.by]`).

As built (arm64, AAPCS64, ELF): values are held in `x0` to `x14`; `x16` and `x17` are the
framework's own moves, `x15` builds offsets and addresses too far for one instruction, and values
of loops take `x19` to `x28`. The prologue saves `x29` and `x30` as a pair, points `x29` at them,
and builds the frame's size in `x16` by two moves the function's end rewrites (instruction and
listing alike); homes are reached from `x29` by `ldur` and `stur`, or at an index past 256 bytes.
Constants take one move where a form holds them (`movz`, `movn`, a bitmask), else a move of the
first differing 16 bits and a `movk` of each other; a symbol's address is `adrp` and `add` of its
low 12 bits, an import's is read from the global offset table. Snippets are three-address, so the
encoder gives a result the register of an operand the step reads for the last time. Narrow
integers compute at 32 bits with undefined upper bits; division, right shifts, comparisons and
conversions of 8 and 16 bits extend their operands first (`uxt`, `sxt`, one instruction per
source width in the table), and have snippets of their own. Floats are held as bits and computed
in `v0` and `v1`; an ordered comparison is one condition of `fcmp` but for `!=`, which is two, so
every other float comparison fuses with its branch. Atomic read-modify-write operations and
compare-and-exchange are loops of `ldaxr` and `stlxr` (Armv8.0, which every arm64 processor
runs); ordered loads and stores are `ldar` and `stlr`. The C convention passes a record of up to
four floats of one type a field per floating-point register, another of up to 16 bytes in up to
two general registers, a larger one as the address of a copy the caller makes, and a result in
memory through `x8`; narrow integers are extended to 32 bits both ways. The backend tests run
every program on linux.arm64 too, through the runner of the machine's toolchain file (spec 12,
section 6), and require the same results as on x86-64.

**Legalization.** Each backend declares the widest integer it computes with
(`BackendCaps::int_bits`); before it reads a unit, `Unit::legalize` splits every integer twice as
wide into halves (low first, placed in memory by the target's byte order): add and subtract with
carries, multiply through quarter-width partial products, shifts by any amount below the width,
comparisons by the high halves then the low ones, extensions, truncations, selects, loads,
stores, block parameters, switches, and calls, a wide parameter passed as its halves and a wide
result as a record of them (returned in registers under the C convention, as C returns its
128-bit integers, through memory under Vun's). Division and remainder call a helper function the
pass adds (restoring division over pairs, signs put back around it). Atomic accesses of wide
integers and conversions between them and floats are refused (`backend.lacks`). A backend that
does not compute `f16` (`BackendCaps::half` is `Half::Promoted`; both the C and the native
backend) receives each `f16` as its bits, an `i16`: every operation on it widens its operands to
`f32` exactly, computes there and rounds back once to nearest even, which gives the `f16` result
since `f32`'s 24 bits of significand are at least twice `f16`'s 11 plus 2; comparisons compare
the widened values. Both sequences are straight-line integer and `f32` operations. A promoted
`f16` passed under the C convention, and a conversion to `f16` from anything but `f32`, which
would round twice, are refused.
`vun build --backend native` links the object with the target's linker; the backend tests run
every program through both backends and require the same result.

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
  back/    api  asm  x86_64  arm64  object  codegen  native  c  cranelift  qbe  llvm  external
  link/    api  wild  mold  lld  gnu  msvc  external
  render/  short  annotate  ariadne  json  sarif
  tools/   build  fmt  bind  clang  probe  harness  driver  lsp  cli  generate  xtask
lib/
  lang/     primitives (declarations with `= _`), syntax/grammar.vun
  std/      prelude, iter, task, mem, sync, fmt, text, meta, diag, lints, log, hw, os, simd, testing, build,
            collections
  targets/  a namespace per system, a target per CPU: targets.<system>.<cpu>
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
- Logging goes through `vun-trace` and is off unless `VUN_LOG` asks for it (`VUN_LOG_FORMAT`
  chooses the layout); a level says how much a line stands for in every crate: `info` a step of
  a run, `debug` a unit of work, `trace` an element such as a statement or an instruction.
  Corpus and snapshot tests go through `vun-harness` (`VUN_BLESS`).
- Timings are activities timed where they run: the query engine times every computation by its
  query's name, and the driver each stage of its table and each build step. Each activity sums
  its runs, its own time (without the activities run inside it) and its total time over every
  thread. `VUN_TIMINGS=1` or `--timings` reports the table at exit, the most own time first;
  when off, timing an activity costs one relaxed load.
- There is one frontend: the LSP runs on the same query engine as the compiler.
- Command lines are declared, never matched by hand: `vun-command` holds command trees (names,
  aliases, typed parameters with their values and variables, shared option sets, examples,
  subcommands, actions) as data, and parses, validates, renders help and completion scripts, and
  reports usage errors as diagnostics with suggestions, from them. Options declared on a group
  or in a set apply to every command below it (`vun debug --diagnostics short cst f.vun` and `vun
  debug cst f.vun --diagnostics short` are one request), and a program's `configure` step applies
  them before the command runs. `vun-driver` declares the `vun` tree; `xtask` and `generate`
  declare their own with the same crate, and each binary's `main` is `Program::main`.
- Terminal output goes through `vun-term`: text is built from pieces naming their `Style`, one
  palette maps styles to colors, tables align columns, and a `Shell` holds standard output
  (results) and standard error (status lines, diagnostics), each colored as `--color` decides.
