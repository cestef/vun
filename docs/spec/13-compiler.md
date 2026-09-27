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
  an input's interface unchanged starts no revision. The inputs are each package's description,
  keyed by its `PackageId`, and each file's text, keyed by its `FileRef`: the typed identities
  spans hash by (spec 10, section 1; decision 247). A query that reaches itself gets the value
  its `cycle` rule gives, with the path of queries, instead of looping. A value computed from
  that cycle value, directly or through what it read, is provisional: it serves the computation
  that asked for it and is not kept, since the query reached again may have another value once
  computed, and which query of a cycle is asked for first must not change what the others are;
  only the first query of the cycle, the one reached again, keeps its memo. Verifying a memo
  that reaches itself again through its reads recomputes it, recovering from the cycle as
  computing does.
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
  The place, borrow and panic checks run on every function of a file and on every generic
  instantiation its program reaches, since a type argument can make a body wrong.
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
  An `offset` of an address stays inside the object its base points into (a slot, a data object,
  or memory reached through a pointer), or one past its end; an offset leaving it is undefined.
  Backends may lower it as pointer arithmetic (the C backend's `+`) and passes may rely on it; the
  verifier rejects an offset whose object and displacement it knows, a chain of constant offsets
  from the address of a slot or a data object, when it leaves that object (decision 245).
  An inline assembly block (spec 07, decision 258) is one MIR statement (`Rvalue::Asm`) and one
  LIR instruction (`InstKind::Asm`), both carrying the block's typed instructions
  (`vun_inline::Block`) over its bindings: in MIR the operands it reads and the places it
  writes, in LIR the values it reads and the addresses of the places it writes, and in both the
  addresses of the declarations it names. It defines no value. The place checks read its inputs
  and in-out places, then write its outputs; the borrow check takes an output as an exclusive
  access; passes take it as acting and impure (`InstKind::acts`, `InstKind::pure`), so nothing
  moves across it and it is never swept. A block that does not return is the last statement of
  its block, whose terminator is unreachable, and both verifiers check it; LIR's also checks that
  outputs are addresses. Clobbers are not carried: the backend derives them from the forms.
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
  (`lang.float.rem` runs as `lang.float.soft.rem`), compiled like any other. A slot only plain
  loads and stores of one scalar as wide as it reach, by its address alone, is then promoted
  (`Function::promote`): its stores become definitions, its loads the definition reaching them,
  through block parameters where definitions meet (as Braun et al. build SSA, every predecessor
  known), parameters passed one value are that value, and the slot goes (`Function::compact`
  renumbers what remains). A read before any store reads zero, as a constant at the entry. Every
  backend then holds those locals as values; passes over LIR rewrite blocks in reverse
  postorder, so a definition is seen before its uses. The builder folds as it builds: an
  integer operation, comparison or conversion of constants is the constant it gives where the
  operation is defined on them (`Domain`), a select on a constant is the value it picks, and an
  identity on the right (`x * 1`, `x + 0`, `x & ~0`) is the left operand and an absorbing one
  (`x * 0`, `x & 0`, `x | ~0`) the constant, a constant operand of a commutative operation
  goes right, and a comparison, 0 or 1, masked by a constant is itself or 0 by the constant's
  low bit; floats are left to the machine.
  After promotion the same folds run over each function (`Function::fold`), since a load
  promoted may now be the constant stored, and a branch or switch on a constant becomes a jump.
  Jumps are then threaded (`Function::thread`): a predecessor passing a constant to a block
  deciding on that parameter, by branching or switching on it or on its comparison with a
  constant, nothing else it computes read beyond it, jumps where the block would send that
  constant, as the join of `if c { true } else { false }` or the head of a loop over a state
  enum would; when blocks after it read its parameters, they are demoted to slots first, each
  jump to it storing its arguments and each read loading, for promotion to rebuild; a jump to a
  block holding only constants read nowhere else and jumping on is moved to where it jumps;
  and a block whose only predecessor jumps to it is merged into it. A slot of an aggregate
  reached only by plain loads and stores at constant offsets and by copies is scalarized
  (`Function::scalarize`): cut into tiles, one per scalar read or written in it and the widest
  aligned integers between (or the scalar a slot copied to or from has there), agreeing in
  size with the slots it is copied to or from (at most 16 tiles), each
  copy touching it a load and a store per tile, and each tile a slot promotion then takes. A
  pure instruction (`InstKind::pure`: no memory read or written, nothing called) equal to one
  dominating it reuses that one's value (`Function::reuse`, walking the dominator tree of
  `analysis::Dominators`, commutative operands in one order), and a block entered by one edge
  of a branch or switch reads what the edge implies: the condition 0 past `else`, a comparison
  1 past `then`, `x` the constant `k` past `x == k`, a switch's value its case. Truncations are narrowed
  (`Function::narrow`): the low bits of a sum, difference, product or bitwise operation are
  that operation on its operands' low bits, so a truncation of one read nowhere else computes
  it at the narrow width, when the operation is wider than the target's addresses or an
  operand narrows without a truncation of its own (a constant, a widened value);
  `(x * k).wrap(u64)` of a `u64` is one 64-bit product. What nothing needs is swept
  (`Function::sweep`): unreached blocks go, a parameter only passed back to itself goes with
  its arguments, one passed a single value is that value, and an instruction that neither writes memory, calls nor reads memory as
  written (`InstKind::acts`) goes when nothing reads its value; lowering guarantees each
  operation's domain, so no trap goes with it. These passes run in rounds, at most four, until
  one changes nothing. `Unit::simplify` takes the functions callees first
  (`Unit::callees_first`), so each is done before a caller inlines it: after its rounds, its
  small callees (internal, of Vun's convention, at most 96 instructions besides constants,
  which code generation computes where used) are inlined (`Unit::inline_into`), the call's
  block ending in a jump to a copy of the callee's blocks whose returns jump to what followed
  the call, instructions keeping their origins, and the rounds run again. A backend emits no code for a function nothing reaches (`analysis::Reached`: exports, addresses
  taken, and what they call), as one inlined everywhere is.
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

**The formatter.** `vun fmt` (`vun-fmt`) changes whitespace only: a file's tokens, comments
included, are the same before and after, and formatting twice changes nothing. It keeps the line
breaks the author wrote, and computes everything else from the CST and the grammar's tables, so
the syntax is never stated a second time and there is no option:

- *Lines.* At most one blank line in a row; none at the start of the file, after an opening
  delimiter or before its closer; the file ends with one newline.
- *Spaces within a line.* One space between two tokens, but none: inside the delimiters whose row
  of `delimiters` is `tight` inside (`(a)`, `[i]`, `#[test]`; `{ x }`, `.{ .x = 1 }` are `spaced`),
  or in an empty group (`()`, `.{}`); before a token of `separators` (`,`, `;`, `:`); after a
  prefix operator (`-x`, `&mut x`, `$f`) unless it is a keyword; before a postfix operator, a
  member's `.` and the opening delimiter of a call, an index or a typed literal (`f(x)`, `a.b`,
  `p.*`, `Point{ }`); after the `.` of `.name`; and around the operators of an infix level whose
  row of `infix` is `tight` (`0..n`).
- *Indentation.* Four spaces a level. A line inside a group is indented one level past the line
  holding the group's opening delimiter, and the line starting with its closer as that line. A
  line that does not start an entry (an item, a statement, a member, a variant, an argument, a
  field, an arm, or what docs and attributes mark) continues one and is indented one level more;
  a raw string line continues the one above at its indent.
- *Comments* keep their place: one space after the token a trailing comment follows, or on a line
  of their own, indented as the line after them (inside a group when that line closes it).

A file that does not parse is not formatted, and its diagnostics are reported.

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
  rebuilt. Compile-time code reads the chosen target as `lang.target.current`, the value of its
  declaration (`Declared`: the `Spec` read, and the value it was read from), and picks what
  differs between systems by its members: `std.os.libc` finds its system's row by
  `current.os`. An address is 16, 32 or 64 bits wide; any other `pointer_bits` does not read.
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
  budget, a constant of the task (`BUDGET`, bytes of the stripped binary on the host), 20 MiB,
  raised from the first measured build (6,382,768 bytes) once the C and native backends took it
  to 7,658,304, and from 10 MiB once the native backend in TPDE's layers took it to 11,172,560, and from 12 MiB once the build passed 12.2 MiB and the https client of
  decision 248 added about 280 KiB (decision 252);
  raising it, like a new external crate, is a decision stating the cost.
- **The link plan is abstract.** Memory regions and sections from the build go into `LinkPlan`; the
  lld and GNU ld adapters turn them into linker scripts, others into flags.
- **Capabilities are checked, never faked.** A program needing guaranteed tail calls, frame sizes or
  vectors on a backend that lacks them is rejected with a diagnostic naming the backends that can.

## 8. Backends and linkers

| Backend | Role | Targets | Notes |
|---------|------|---------|-------|
| C | reach | anything with a C99 compiler | first; vendor toolchains, Cortex-M, AVR, Xtensa; strict C99, defined C only (section 8.2); `restrict` from aliasing facts; frame sizes via `-fstack-usage` |
| Cranelift | in-process debug backend (still to come) | x86_64, arm64, riscv64, s390x | in process; roughly 20% less codegen time than LLVM for rustc, code about 14% slower |
| QBE | small and simple | amd64, arm64, rv64 | subprocess on IL text; no vectors (lowered), no tail calls, no 32-bit targets |
| LLVM | optimized builds, widest native reach | ARM32/Thumb, RV32, AVR, wasm, ... | optional; `noalias`, `musttail`, stack-size sections |
| native | fastest debug builds | x86_64, arm64 | the toolchain's own: a single-pass LIR backend in the style of TPDE (8x to 24x faster than LLVM -O0), section 8.3 |
| wasm | WebAssembly without a C toolchain | wasm32 | the toolchain's own: relocatable objects `wasm-ld` links, section 8.4 |
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
  calls, naked functions, and, where the compiler offers nothing for them, atomics (without its
  builtins, `lang.target.Atomics`) and data aligned past every C type or placed in a link section
  (without GNU attributes, `lang.target.Attributes`: `__attribute__((aligned(n),
  section("name")))` on the object) (`backend.lacks`).

- **Aliasing.** An address a signature passes says what else reaches what it reaches
  (`AbiType::Address(Alias)`): an exclusive borrow is `Alias::Exclusive` (spec 05, section 8), and
  C declares it `restrict`.
- **Shards.** A program of more functions than a shard holds on average (`SHARD`, 256) spans a
  header every file includes (types, checks, declarations), a file of its data, and shards of
  functions, each function in the shard its content hash gives, so the files compile in parallel and a changed
  function recompiles one shard. Its functions and data are then seen by the linker, under names
  tagged by the unit's content hash (`p1f3a_f3`), never another program's.

The C compiler is target data (`lang.target.CCompiler`): its command, the flags the rules above
need (`-std=c99 -ffreestanding -fno-strict-aliasing`: the output is freestanding C, declaring all
it uses, so no function name means anything to the compiler), the flags its links take (bare
metal: `-nostdlib -nostartfiles`), its atomics, and how it reports frame sizes
(`lang.target.FrameSizes`). `vun build` writes the backend's artifacts under
`.vun/build/<content hash>/` next to the root file, the hash of the artifacts, the C compiler
and the profile, compiles each source with it through `vun-tool`, in parallel, with
`-fstack-usage` where it has it (`vun debug frames` prints the sizes read back,
`Backend::frame_sizes`), and links with the target's linker. What the directory holds is kept:
an object is made under another name and renamed once whole, so a build finding it compiles
nothing; a link leaves a stamp named by the hash of the linker, the target and the plan, holding
the hash of the output it wrote, so a build finding the output unchanged links nothing. The `gnu` adapter drives
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
- **`vun-object`**: relocatable objects as data, written once in the target's format: ELF-64,
  or Mach-O 64-bit for arm64 (`macos.arm64`), whose symbols are C names with a leading
  underscore, whose relocations keep a word's addend in place and an instruction's in an
  `ARM64_RELOC_ADDEND` before it, and whose debugging sections are `__DWARF` ones; relocation
  kinds are the framework's, each format and machine numbers them. A target whose format the
  native backend does not write for its machine is refused (`backend.lacks`). Each
  architecture's instruction compilers take the target's variant of the C calling convention:
  on arm64, AAPCS64 or Apple's, whose C calls pack scalars on the stack at their own size.
- **`vun-dwarf`**: debugging information (DWARF 4) as sections of an object: one compilation
  unit and the line table of every function, a row per instruction whose line changes, from the
  marks the framework records (`Compiled.marks`: each instruction's and terminator's code with
  its origin, the prologue with the function's), addresses as relocations against the
  functions' symbols. Debuggers break on and step through Vun lines; `addr2line` and profilers
  map code to them. An internal function's local symbol is named by its declaration path
  (`Pair.swap`), which instantiations share. Call frame information (`.eh_frame`, ELF objects)
  describes each function by the steps the framework records (`Compiled.unwind`: frame pointer
  and return address stored, frame pointer set, callee-saved registers saved, and around each
  return the frame torn down), each machine giving its registers' DWARF numbers (`Arch::dwarf`)
  and where its calls leave the return address (`CallFrame`); unwinders and profilers walk
  through Vun frames at every instruction.
- **`vun-regalloc`**: the register allocator (section 8.6), which knows no architecture.
- **`vun-codegen`**: the framework, which knows no architecture: the plan and its moves, the
  frame, homes, values recomputed where used, the callee-saved registers the plan uses, compare
  and branch fusion, and the snippet encoder with what snippets need. An architecture gives it
  its registers and its own moves (`Arch`), and its instruction compilers with what each
  instruction needs (`Isel`).
- **`vun-backend-native`**: per architecture, the calling conventions, the snippets and the
  instruction compilers; the unit assembled as one object; listings.

The framework:

- **One pass per function**, once the allocator has planned where values live. Blocks in
  `Order`, instructions in program order; instruction selection and encoding happen together
  into a code buffer that later steps only patch (jumps to later blocks, the frame size).
- **Values.** Each value has an assignment, indexed by its id: its home in the frame, once one is
  needed, and its uses. Constants and addresses known while compiling (a slot's, a datum's, a
  function's, at constant offsets) are recomputed where used, never kept: `Recompute`, found
  once per function; the plan holds them nowhere, and an instruction needing one in a register
  writes it into a scratch register once per instruction.
- **Registers.** Where each value lives is planned before the pass (section 8.6): each
  instruction's architecture says what it needs, one decision (`Selection`) giving both what it
  needs and what it emits, and the framework follows the plan: the moves before and after each
  instruction and on each edge, a block starting with values where the plan has them, and the
  callee-saved registers the plan uses saved by the prologue. A conditional jump whose edge moves
  anything goes there through its own moves.
- **Instruction compilers** ask the framework for operands (`Cx::reg`), results (`Cx::def`), a
  result overwriting an operand (`Cx::reuse`: no copy where the plan gives both one register),
  and scratch registers (`Cx::scratch`), each where the plan has them; a mismatch between what an
  instruction said it needs and what it asks panics rather than miscompiling.
- **Snippets** (TPDE, section 4): most operations are instruction sequences as data, over slots
  for the operation's operands (`In`, and memory at an operand's address, `At`), its result
  (`Out`), scratch registers (`Temp`), registers an instruction requires (`Fixed`, taken for the
  whole instruction) and operands taken as an immediate where a form takes one, else through the
  register the instruction requires (`Through`: a shift's count, in `cl` unless constant). What a
  snippet needs is read from its slots (`Cx::snippet_needs`): each operand in a register, read
  again after the result is written when a later step reads it; the result in the register of
  the operand its first step moves into it, or in the fixed register it moves it from. The
  snippet encoder places each operand as the forms allow: a constant as an immediate, a known
  address folded into a memory operand, else in its register; and a move of an operand into the
  result is no instruction where the plan gives both one register, as is a move of a register to
  itself at the full width (narrower ones clear the bits above). Where TPDE derives snippets from C through LLVM,
  they are written against the instruction table: `add` is a move of the first operand into the
  result then `add` of the second, and the encoder turns that into `add rdi, 0x1` where the first
  operand dies in `rdi` and the second is 1.
- **Fusion** looks only forward within a block: a compare whose one use is the branch ending it
  is made by the branch, which jumps on the flags. A branch whose `then` block is emitted next
  jumps where the condition does not hold and falls into it (`Layout`), so a branch takes one
  jump and never jumps over the block after it.
- **Frames.** Always a frame pointer; the prologue reserves the frame with the widest immediate,
  its size written, and listed, when the function ends, and the callee-saved registers taken are saved below
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
`r10` and `r11`, which the framework keeps for its own moves, then in those a call preserves,
`rbx` and `r12` to `r15`. Vun's convention passes the signature's values in the registers C passes
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
framework's own moves, `x15` builds offsets and addresses too far for one instruction, and
values are also held in `x19` to `x28`, which calls preserve. The prologue saves `x29` and `x30` as a pair, points `x29` at them,
and builds the frame's size in `x16` by two moves the function's end rewrites (instruction and
listing alike); homes are reached from `x29` by `ldur` and `stur`, or at an index past 256 bytes.
Constants take one move where a form holds them (`movz`, `movn`, a bitmask), else a move of the
first differing 16 bits and a `movk` of each other; a symbol's address is `adrp` and `add` of its
low 12 bits, an import's is read from the global offset table. Snippets are three-address, so a
result may take the register of an operand the step reads for the last time, as the plan gives
it. Narrow
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
stores, block parameters, switches, and calls. A wide parameter is one `AbiType::Wide`, its
halves two values that each backend's call classifier places together by its convention (an even
register pair under AAPCS64, two registers or 16 aligned bytes of the stack under System V, as C
passes `__int128`; decision 246), and a wide result is a record of them (returned in registers under the C convention, as C returns its
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
`vun build --backend native` links the object with the target's linker, or the one `--linker`
(`VUN_LINKER`) names, refused with `link.unsupported` when it does not link the target's format
or regions; the backend tests run every program through both backends and require the same
result. GNU ld, mold and wild link, all through the target's C compiler, which passes the C
runtime's start files and libraries (`vun_link_gnu::Ld`): its own `ld`, `-fuse-ld=mold`, or wild
as the `ld` of a directory beside the output that the compiler looks in first (`-B`).

### 8.4 The wasm backend

The toolchain's own backend for WebAssembly (`wasm32` targets, `wasi.wasm32`), in one crate,
`vun-backend-wasm`. It writes a relocatable object in the format of the WebAssembly tool
conventions (a module with the `linking` and `reloc.*` custom sections), which `wasm-ld` links
(the `lld` adapter, flavored by the target's object format), so its objects link with C compiled
by clang for the same target.

- **Modules as data.** A module is a typed value (`Module`: types, imports, functions, data
  segments, symbols, relocations), written once into bytes; instructions are a typed set
  (`Op`), each declaring its encoding and the operand types it pops and pushes. `Module::verify`
  checks every index and runs the validation algorithm of the WebAssembly specification over
  every body, so a module the backend makes is valid before it is written; tests also validate
  with an independent parser (`wasmparser`).
- **Values.** Integers up to 32 bits and addresses are `i32`, 64-bit integers `i64`, floats
  their own (`BackendCaps::int_bits` is 64, `f16` is promoted). A value narrower than 32 bits is
  held zero-extended: every operation producing one masks it, sign-reading operations
  (`SLt`, `AShr`, `SDiv`, `SExt`) extend it first (`i32.extend8_s`). Each LIR value is a local,
  each block parameter one too.
- **Control.** Blocks become structured control by the algorithm of Ramsey (2022), from the
  dominator tree and reverse postorder: a block with more than one forward predecessor is the
  continuation of a `block`, a loop head a `loop`, a jump a `br` to the label of its target, a
  switch a `br_table`. A jump's arguments are pushed, then popped into the target's parameters in
  reverse, which is a parallel copy with no temporary. A function whose control flow is
  irreducible (a coroutine's resumption enters its loops in the middle) runs as one `loop` around
  a `br_table` on a local naming the next block, each jump setting it and branching back.
- **Memory.** Stack slots live in linear memory below `__stack_pointer`, a global the linker
  defines: a function with slots lowers it on entry by its frame, rounded to 16 bytes, and
  restores it before each return. `Copy` is `memory.copy` (bulk memory). Atomic accesses are
  plain ones, since a module without shared memory runs one thread.
- **Calls.** Vun's convention passes scalars as values and aggregates by address; the C
  convention is clang's for wasm32: a record of one scalar is that scalar, another is passed as
  the address of a copy the caller makes and returned through an address passed first; a wide
  integer is two `i64`s. An indirect call is `call_indirect` through the linker's
  `__indirect_function_table`, a function's address its index there.
- **Imports.** A linked symbol is an undefined function or data symbol of the object. The library
  it comes from, `#[link.library(name)]`, is its import module (`wasi_snapshot_preview1`), and
  the linker keeps it an import of the program; a symbol with no library is one another object
  defines.

### 8.5 Calling conventions

Where a call passes each value is decided once, in `vun-abi`, from the convention as data, and
every backend that places arguments itself reads the result; no backend states a convention's
rules (proposed, decision 256).

- **Conventions as values.** A `Convention` declares: how many argument and result registers
  each class has (general, floating-point); where the address of a result in memory goes (the
  first general argument register, System V, or a register of its own, `x8` under AAPCS64);
  how records travel (`Records`: System V's eightbytes, each general or SSE by its fields; or
  AAPCS64's homogeneous aggregates of up to four floats of one type, a field per register, and
  other records of up to 16 bytes a word per general register); what becomes of a larger record
  (copied onto the stack whole, or passed as the address of a copy the caller makes); whether a
  record that finds too few registers of its class leaves the rest of the class unused
  (AAPCS64, rule C.13) or takes none and leaves them to later arguments (System V); where an
  integer of two words goes (any two general registers, as System V and Apple's variant take
  them, or an even pair, AAPCS64 rule C.9);
  the stack (a slot of 8 bytes, or scalars at their own size and alignment, as Apple's variant
  packs a C call's; composites aligned to at most 16 bytes); who extends a narrow integer
  (the callee reads it extended by the caller, as Apple's variant says, or extends it itself);
  and whether a variadic call counts the floating-point registers it uses (System V's `al`).
  `SYSV`, `AAPCS64` and `DARWIN_ARM64` are the three built; riscv64's LP64D and wasm32's C
  convention are further values of the same type.
- **One classifier.** `Passing::of(signature, convention)` gives every LIR value of a signature
  (`Signature::passed`: a wide integer's halves each, the address of a result in memory last)
  a typed placement: a register, a place on the stack, a record's parts in registers with their
  offsets and sizes, a record copied onto the stack, the address of a copy the caller makes and
  where that address goes, or the address a result returned in registers is written to; and
  the result its own: none, a register, parts in registers, or memory through an address. It
  also gives the bytes of arguments on the stack. Registers are named by their class and
  position among the convention's argument or result registers (`Reg::Int(2)`); the backend
  maps positions to its machine's registers, so the crate depends on no assembler, and
  registers a platform reserves (Apple's `x18`) are the backend's to keep out of allocation.
- **Vun's convention** passes each LIR value of the signature in the general registers in order,
  then on the stack, records by address and a result in memory through an address passed last;
  a wide integer as the C convention of the target passes one.
- **Chosen by the target.** Each target description states its convention, a
  `lang.target.Convention` (`sysv`, `aapcs64`, `darwin_arm64`, `lp64d`, `aapcs_vfp`, `ilp32`,
  `wasm32`), so no backend matches a target's ABI by name; an architecture lists the conventions
  it follows, and a target naming another is refused (`backend.lacks`).
- **Checked against clang.** For each built convention, a C function per argument returning it
  is compiled by clang for the target, and the register or stack slot its code reads the
  argument from must be the classifier's.
- **Readers.** The native backend's instruction compilers read placements for calls, entries and
  returns. The C backend reads none: the C compiler places arguments, which is what the
  classifier is checked against. Tests compare placements with what clang does for the same C
  signature on each target, and the backend tests run every `#[abi.c]` function through the C
  and native backends and require the same results.

### 8.6 Register allocation

The native backends share one register allocator, `vun-regalloc`, which plans where every value
of a function lives before the framework compiles it; the framework then follows the plan
instead of choosing registers as it goes (proposed, decision 257). Against the framework's
earlier choices as it went, the backend test programs take 3 to 22 percent fewer instructions
and up to 85 percent fewer frame accesses (`half.vun`: 13,238 instructions to 11,686, 1,774
frame accesses to 264); the LCG loop of decision 230 runs its 50 million rounds in 7
instructions each instead of 13, in registers a call may overwrite, none saved.

- **Linear scan on SSA**, after Wimmer and Franz (2010): LIR is in SSA form with block
  parameters, so each value's lifetime is built in one backward walk of the blocks in `Order`,
  as intervals of positions with holes, and its uses with what each needs. Intervals are
  allocated in order of their start. A value that finds no register free for its whole
  lifetime is split: the part up to where the register is next needed keeps it, the rest waits
  for another, and a part that finds none is spilled to a home in the frame, split again before
  a use that needs a register. Values recomputed where used (constants, addresses known while
  compiling, section 8.3) have no interval.
- **Constraints as data.** An architecture describes its registers once (`Machine`): the banks,
  each with its allocatable registers in the order they are preferred, the registers a call
  preserves, and those the framework keeps for its own moves. Each instruction states what its
  operands and result need (`Needs`): any register of a bank, a fixed register, a register or
  memory, or the register of one of its operands for a result that overwrites it; the registers
  it writes besides its result; and the scratch registers it takes. Snippets give these from
  their slots (`Fixed`, `Through`, `Temp`), and calls from the placements of `vun-abi` (section
  8.5) and the convention's registers that a call does not preserve. A fixed register is an
  interval of its own that the values meeting it are split around.
- **Preferences.** A value crossing a call prefers a register the call preserves, and a value
  passed to a block parameter, an argument register or a result prefers where it goes, so that
  most moves disappear.
- **Resolution.** Where an edge leaves a value in one place and its target expects it in
  another (a block parameter, or a value split between them), the moves are made on the edge
  as one parallel move (`Moves`), in the predecessor when it has one successor, else in the
  successor when it has one predecessor, else in a block of their own.
- **The plan** (`Allocation`) gives each value its place at every position (a register or its
  home), and the moves at each position and edge. The framework compiles in one pass as before,
  asking the plan where an operand is and where a result goes; a block starts with values where
  the plan says, not in their homes, and a callee-saved register is saved by the prologue when
  the plan uses one.
- **Checked.** A checker simulates the plan over the function's control-flow graph: what each
  register and home holds, through definitions, moves, spills, reloads, clobbers and edges,
  meeting at joins by what every predecessor agrees on, and requires every use to read the
  place holding its value. It runs on every function in tests and in a fuzz target (`fuzz/`,
  `regalloc`).

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
  base/    index  span  intern  origin  diag  query  pass  tool  target  trace  items  json  codec
           codec-derive  hash  graph  fs  outline  par  suggest  term
  front/   grammar  desugar  syntax  hir  sema
  mid/     mir  passes  interp
  low/     lir  lower  abi  passes
  back/    api  asm  x86_64  arm64  object  dwarf  codegen  native  c  cranelift  qbe  llvm  external
  link/    api  wild  mold  lld  gnu  msvc  external
  render/  short  annotate  ariadne  json  sarif
  tools/   build  pkg  http  fmt  bind  clang  command  harness  driver  lsp  cli  generate
           xtask
fuzz/      cargo-fuzz targets over the verifiers, a workspace member
lib/
  lang/     primitives (declarations with `= _`), syntax/grammar.vun and syntax/desugar.vun
  std/      prelude, option, result, cmp, int, iter, seq, ptr, task, mem, sync, fmt, text, meta,
            diag, lints, log, hw, os, simd, panic, testing, build, collections
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
