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
 Linker (trait)        wild | mold | lld | wasm | GNU ld | MSVC link | none
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
  memos are. Before the reads of a memo reading many are verified in order, the memos they
  reach are verified ahead over threads, computing none: each whose reads all hold is marked
  current, so verifying in order finds it done, and computes only what changed, in the order
  that never computes a read keyed by an earlier read's value before that value holds; one
  thread looks at each memo, another reaching it leaves it to verifying in order. Durability
  (as Salsa's): an input is set as the database's own, as shared among databases that chose
  alike (the target, the checks) or as shared by all (a library's sources), and a memo is as
  durable as the least shared input it reads through all it read. A memo is saved with the
  fingerprint of every input of its level or a more shared one when it last held, and holds at
  once, its reads never looked at, while that fingerprint is the same: a library's memos hold
  whatever the target, those reading the target while it is the one chosen. An input not set
  stays out of every fingerprint, so a memo that read it unset never matches one that read it
  set. Under
  `VUN_QUERY_CHECK=1` what verifying ahead finds is only recorded, and verifying in order fails
  on a memo it found to hold whose reads changed. Traces hold fingerprints rather than revision
  numbers, so they stay valid in the next process. Each key gets a slot in its query's table
  when first asked for; reads are recorded, verified and compared by table and slot. Threads share a database without locks on
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
  key position in `.vun/queries/<toolchain>` of the root file's project (spec 12, section 5), the toolchain named by its
  executable. A memo whose trace reads a query that is not persisted is not saved; inputs
  (`Query::INPUT`) never are. A loaded memo is verified before its first use like any memo of an
  earlier revision, so a stale file costs recomputation, never a wrong result; a traced memo
  that holds stands for its value, which is computed only if read (red-green, as rustc's
  dependency graph). Loading reads what is used, in place in the file: a query's keys when one
  of them is first looked for, found by the stable hash of the key's value (`vun_hash::Stable`,
  which symbols, interned values and origins hash by), a key's memo when that key is first asked
  for (a key reached through another memo's reads gets its slot by its position, and is read
  when first needed), a memo's reads when it is first verified and its value when first needed; so a run pays
  for the memos it uses, not for those of other programs sharing the file. Saving happens only
  when a memo of a persisted query was computed that the file did not hold, and extends the loaded stream: every key
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
- The libraries' memos are shared between projects. An input is set as the project's own
  (`Db::set`) or as one every database sees alike (`Db::share`, `Sharing::Shared`): the files and
  package data of every package but the project and loose files, the target chosen and the
  checks; an input computed while unset is alike everywhere too. A memo is shared when every
  read leads to shared inputs alone (`Db::save_shared` finds them, reads of a query not
  persisted or of the project's inputs excluding a memo). They are saved whole into a file of
  the store's `memos` directory (a directory the tests of one build share, for them), in a
  directory named by the toolchain and one named by the target chosen and the checks, the file
  named by the hash of its body, and written once: by a session that loaded no shared file, when
  it ends. A session that loaded one saves the shared memos it lacks with its own memos, layered
  over it. A memo counts as computed only when no memo loaded equals it
  (`Novelty`): one read again for its value, as a traced query's, is not new. Files older than
  a week but the newest are removed, and those of other toolchains. Loaded memos are verified
  before use, so a shared file never makes a result wrong, only a build faster: `lang` and `std`
  are checked and built into MIR once per toolchain, target and checks.
- A project's memos are saved layered over the shared file it loaded (`Db::load_over`), naming
  it by its hash: the stream continues that file's (places and pool numbers past its own), each
  query of that file keeps its position, and each query's keys follow that file's, so that a read
  saved in either names the same memo in both. The project file holds the keys the shared file
  lacks, and the memos of its keys computed again (`replaced`); it extends itself as a file alone
  does, and is written whole over the shared file once it outgrows its size. A session with no
  file of its own loads the newest shared file; one that loaded nothing takes the shared memos it
  saves as the file its own are layered over (`Taking::Written`: each key takes the place the save
  gave it, nothing read back), so a first build is layered too.
  A project file whose shared file is gone is dropped, its memos computed again.

**Parallelism.**
- Independent work goes through `vun-par`: `par_map` over items, results in order, on the
  process's pool of threads, started once. A call is open to the workers while its caller runs
  its own share: any worker free, at the call or later, joins the newest call open with room,
  calls made inside another's work included, so it waits on nothing queued behind other work.
  The pool names the workers in each thread's calls (`Pool::helpers`), and a thread about to
  wait on a query's claim follows those as it follows claims, computing the query itself when
  the path comes back to it. `get_all` computes each key on a context under the caller's path
  of queries, so a cycle through it is found as on one thread. The pool is the one place of the
  compiler with `unsafe` code: it lends a closure borrowing the caller's stack to threads that
  outlive the call, sound because workers join only while the call is open, counted under the
  pool's lock, and the call closes and returns, or unwinds, only once every worker counted has
  reported done with it; its tests run under Miri over several schedules.
- Simplifying a unit tidies every function at once, tidying reading no other function, then
  inlines callees first.
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
  (`vun debug dump program` lists them).
- **Typing says why.** Typing gives every form a type, reports why not, or records where it gave
  up silently, with a typed reason: the rule of the form's kind, the type of what a name reaches
  having no value there, or a type whose members are not all known there. A body that is not
  concrete over such a form is reported at it, with the reason; `vun debug dump types` marks each
  (decision 359). The reason travels to what depends on the form: a local bound without a type,
  and a form typed by an expected type its context cannot know, give up for the same reason. A
  form reading a `$` parameter the generic body leaves unbound is left to the instantiations
  (`Unevaluated::Unbound`), and one an error typing reported explains is not silent; no body a
  corpus program reaches, generic bodies included, may hold any other (decision 360).
- **Layouts.** Every type with a run-time layout has a shape (query `Shapes`): its size and
  alignment, the offset of each part, how an enum tells its variants (an index, or a niche
  pattern), and its unused patterns (spec 05, section 6). `vun debug dump layout` prints them.
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
  writes, in LIR the values it reads and the addresses and scalar types of the places it
  writes, and in both the addresses of the declarations it names. It defines no value. The
  place checks read its inputs and in-out places, then write its outputs; the borrow check
  takes an output as an exclusive access; passes take it as acting and impure
  (`InstKind::acts`, `InstKind::pure`), so nothing moves across it and it is never swept. Native
  code overwrites every register a block names, its inputs' too: no value is kept there past it.
  A value bound where a form takes no register, such as an immediate, fits another form. A block that does not return is the last statement of
  its block, whose terminator is unreachable, and both verifiers check it; LIR's also checks that
  outputs are addresses. Clobbers are not carried: the backend derives them from the forms.
  Functions, data and imports are named by id, and only exports and imports carry link symbols.
  Under Vun's convention a value in memory is passed by address and a result in memory written
  through an address passed after the parameters; under the C convention (`#[abi.c]`) such values
  are records passed by value, their scalars given for the backend's classification. A trap site
  calls `lang.panic.trap` with a `lang.panic.Trap` whose `site` indexes the unit's message table.
- **Lowering** (`vun-lower`, query `Lowered`, `vun debug dump lir`) turns a program's concrete MIR into
  LIR one function at a time: each is a query of its own (`FunctionLir`, keyed by the
  instantiation, whether it is a coroutine's `end`, and whether the checks of unsafe code apply
  in it; or by a constructor taken as a value), lowered and tidied with every id it holds a
  position in tables of its own, the functions, link symbols, data and trap sites it names
  (`Refs`). So a function of a library lowers once for every program, its memo shared (section
  2). Assembling the program computes its functions over threads and places them in one unit:
  functions in the order the program reached them (in waves, each wave's bodies made over
  threads, in the order reaching one function at a time finds), coroutines' `end`s and then constructors
  after; a link symbol the function the program exports under it, else one import per symbol;
  constants once per content, a top-level binding once per binding and a trap site once per
  site, each after the data its bytes point to, the ids that data and assembly checks hold
  written as placed (a site's id is never an immediate a pass could fold). Each function placed
  has its small callees inlined already (`InlinedLir`, keyed by the function and the functions
  the checks run in, which stay unchecked wherever they are inlined): the callees, inlined
  themselves, are placed in the function's own tables as the unit places a function
  (`placing`), inlined and the function tidied again, so a library function's is shared as its
  LIR is; a function reached again through its callees inlines none of theirs. A callee's trap
  sites are placed with origins derived from theirs (`Via::Inline { into }`, the function
  inlined into), so a trap names each function its code is inlined into; where the program's
  table describes calls (`lang.trace.Detail.calls`), a callee is inlined only if no call it
  makes returns into it (a trap's call never does), so every return address lies in the code of
  the function the table names for it (spec 07, section 3). Lowering
  makes every local a stack slot of its layout, a top-level binding one data object (of
  `Storage::Thread`, reached with `thread_addr`, where its type marks it `#[thread.local]` and the
  target runs threads), statements the
  instructions computing into their place; integers in their range's storage, converted where a
  value moves between ranges; enums by their shapes; checks of indices and narrowings as trap
  sites. A primitive no instruction computes is marked `#[lang.lower.by(f)]`: making its caller
  concrete rewrites each call of it into a call of `f`, a Vun function of the same parameters
  (`lang.float.rem` runs as `lang.float.soft.rem`), compiled like any other. A primitive marked
  `#[lang.lower.soft(f)]` is rewritten so when a constant argument names a type the target does not
  compute with (`Target::computes`: a float `lang.target.Spec.floats` does not list, an integer
  wider than an address): `lang.float.add` runs as `lang.float.soft.arith.add`. A float
  converted to an integer is brought within the integer's storage before the instruction, which
  hardware and C leave undefined outside it, and the bound put in after. A slot only plain
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
  A constant inside a loop is defined before it (`Function::hoist`), at the end of the block
  immediately dominating the header of the outermost loop around it, irreducible loops left as
  they are, so a backend keeping it in a register keeps it across the loop.
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

The grammar (tokens, node kinds, their productions) is defined once, in `lang/syntax/grammar.vun`
and `lang/syntax/nodes.vun`, as Vun data. From it are generated: the Rust kind enums and typed
accessors used by the parser and every compiler crate; `lang.syntax`, which compile-time code uses
to read code values (spec 03); the formatter's node table; and the tree-sitter grammar for editors.
Tokens and nodes are therefore defined in exactly one place and cannot drift.

`cargo gen` reads the grammar with the compiler's own parser as literal data (the
reader of `build.vun` manifests, spec 12) and translates each declaration by the shape of its
value: a `PascalCase` record of lists is a kind, an `enum` an enum, any other list a table whose
element type is inferred; a table whose rows are records of several fields is a list of a row
struct named after it (`iteration` holds `IterationRow`s), read by field name, and a column of
paths into `lang` (`"lang.ops.Try.branch"`) is generated as their handles, members of type
constructors and interface requirements included. Rust reads what a handle declares through it
(`Item::parent`, `Item::symbol`), and a borrow mode is a row of the table `borrows`
(`vun_desugar::Borrow`), never its words. `grammar.vun` and `nodes.vun` go to `vun-grammar`, which the parser is built
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

What the generator interprets rather than copies, the productions and the tables the parser and
editors share, it reads by type: each is a declaration of a Rust type deriving `FromLiteral`
(`vun_syntax::literal`), and the whole is resolved once into a model naming tokens, kinds and
fields by typed ids, which every emitter writes from; a name or path that resolves to nothing, or
a value of another shape, fails the run naming it. Tables are named by paths to their declarations
(`grammar.infix`, `grammar.TokenKind.keywords`), through a declaration aliasing a file's namespace
(`grammar := lang.syntax.grammar`), so a reference is checked by name resolution, never matched as
text. What editors make of the grammar is declared beside it, in `lang/syntax/editors.vun`: what
highlights capture by kind, table and place, what the outline lists, text objects, the Sublime
syntax's scopes and the few facts the tree-sitter grammar leaves to tree-sitter; captures and
scopes are named as the generator's typed enums declare them.

The same task writes `editors/sublime/vun.sublime-syntax`, the highlighting that editors, the
whitepaper and the README's code card read (through syntect): keywords, contextual words, borrow
words and every punctuation token come from the grammar, each punctuation token scoped as
`editors.vun` says, and the tokens the lexer reads by hand (comments, strings, numbers, labels)
from the lexer's constants.

**Editor grammars.** The same task writes the tree-sitter grammar of `editors/tree-sitter` from
the productions: a rule per kind; a field holds the kind it is named after, the kinds the tables
`holds` (only these) or `admits` (these and any operand) give it, or any operand, the kinds
`parts` lists being none; precedences come from the table `precedence`, whose row naming the kind
spread over the infix levels stands for those levels; a restricted field (`.of`, `.starting`)
holds variants of the kinds reaching an allowed one through their edge field, aliased to the
kind, and, as the parser's `standing_for`, those of the `transparent` kinds ending with one where
no kind it allows continues an operand (a transparent node before a form continuing one holds
the form: `$T{}` is `$(T{})`), and completes its entry where its kind has no level of its own; a
kind of `transparent`
ends with its last child only when written with one of the tokens its row lists (`$`, `unsafe`,
a doc comment, `#[`); where several kinds fit,
the first of `preferred` wins. The sets of rules tree-sitter must keep apart are an artifact of
its tables and are found, not written: `cargo xtask tree-sitter` generates the parser until it
builds (`conflicts.json`), then parses every source and compares each tree with the compiler's;
every source of the repository must agree. Queries (highlights, folds, indents, brackets, the
outline, text objects) are built once with typed captures and written in each editor's dialect,
with the Helix, Neovim, Zed and VS Code packages (`editors/`); what an editor runs as code is
written by hand and reads the generated data. The grammar and its parser are published apart, as
`tree-sitter-vun`.

**Productions.** Each node kind is declared by its production: the tokens and children it spells, in
source order, `.binding(.optional(.\mut), .pattern, ":=", .value)`. A term is a token by its
spelling (`":="`, `"if"`, or a name spelled so where it is a word: `"in"`), `.token(...)` for a
token of the kinds it names (`.token(.ident)`) and of the lists of the token kind or tables of
tokens its paths name (`.token(grammar.infix)`), `.word(path)` for a name or keyword spelled as a
variant of the kind, or the list of one, the path names (`.word(grammar.BorrowWord.modes)`), a child
`.field`, `.optional(...)`, `.many(...)`, `.separated(term, ",")` (with an optional trailing
separator), `.choice(...)`, a sequence `.seq(...)`, or a child restricted by a table: of a kind it
lists (`.of(.field, table)`), or starting with a token it lists (`.starting`). Wherever a table
names kinds (`.of`, `holds`), a `transparent` node stands for the node it ends with: `#[a] x: T,`
is a field as `x: T,` is. A leaf spells one token. A node is anchored at its production's first
token; its other tokens are `punct` leaves, and a token no production reads is a `skipped` leaf,
which stands in no field. The generator derives from the
productions a field enum and each kind's slots: a field under `many` or `separated` holds many
nodes, one under `optional` or `choice` one at most, any other exactly one, and one standing in
several places in a row many; a field named after a node kind holds a node of that kind, any other
field any node. `Cst::fields` assigns a node's children to its slots (left to right, backtracking,
leaving a node to a later slot restricted to its kind). **The parser** reads the productions themselves: each is compiled to an automaton over its
terms, one position per token, word or child term (Glushkov's construction, `vun_grammar::Automaton`),
and a stack of frames reads the tokens, readings of nodes alternating with the operands they read.
A reading keeps every kind the node may still be, each at the positions of its automaton it stands
at; a child is read once for every position reading it, taking what they take merged
(`engine::Spec`: the kinds they name, whether any operand, the parts `admits` lists, the level),
and positions that may not hold what was read drop. One rule says who reads
a token: the outermost reading that reads it where it stands, every frame above it able to end
(so `if x {` opens the body and `if a {} .x;` starts a statement); else the innermost operand a
form may continue (a kind whose production begins with a child the operand may fill, reachable
from what the position takes), at the operand's level of `precedence` or tighter, the one reading
the longest run of tokens before its next child, forms reading as long a run
continuing together (`x: T` is a `typed_binding` or a `field`); else the top frame ends where it
may. A child starts only where a kind it may be starts with the token (its production's first
token or word, or a first child that is no operand, as `mut`), a `transparent` kind standing for one named only with its
tokens, so every move reads a token. An operand is read at a level: an edge child at its parent's
(the infix form's right operand tighter than its operator), an interior one at the loosest, and
where positions reading one child disagree, at the tightest. In a node of a kind of `line_ends`,
and everything read inside it, nothing reads a token after a line break, and such a node holds
on to its line while it reads. A chain of operators of a level without grouping is an `error`
(`chained_operator`), and nesting past `MAX_NESTING` operands, or links of a chain, is one error
holding the rest of the expression. A node's kind is chosen when the tree is written: the
preferred (`preferred`) of the kinds it spells that may end where it ended, that the position
holding it takes along the path its reading took (found back from where it ended); a transparent
node standing for what it ends with passes that on to its last child, whose reading took both
what it takes itself and what the node's position takes, and only the latter where the node may
stand there as nothing else (`.$(e)`). So what a group becomes follows from what it spells and
where it stands: `()` is a tuple, `() {}` a function, `(a: T)` a function type.

Recovery mends the top frame where it may neither go on nor end, each report once per position:
a reading supposes absent what a next position reads where the token may be read after it (a
token, or an `error` child); before the end of the file, the token closing a group it opened, or
a token a frame below reads (within an unclosed group, only as punctuation), it completes along
its shortest way out up to the token (`unclosed_delimiter` where that supposes its closer); else
it skips the token, with its group, as `skipped` leaves. An operand holding what may not stand
there is reported with the tokens the kinds named there would read after it (`:=` after `run()`
among declarations) and goes on as the first of them reading the token after supposing one
(`.some(v) v` among arms), else is held in an `error`. After a report, and after a token the
lexer reported, the parser is quiet until a production reads a punctuation token in place: what
goes wrong in between is the fallout, reported by nothing, and recovery only completes or skips.
What a report expects is read from the positions: their tokens, a name where a position names
`name`, a declaration where one names a kind continuing an operand, else an expression.
Each distinct shape is its own kind, so a
kind's fields never depend on its anchor token: `x := e` is `binding` and `x: T = e` is
`typed_binding`, `.{ }` is `struct_lit` and `T{ }` is `typed_lit`. Malformed source is an `error`
node, which holds what recovery skipped and fits any slot. The CST verifier requires every node to
fit its slots and every node recovery did not build (an `error`, a node holding one or anchored at
an absent token, and a node holding such a node, whose extent recovery decided) to spell its
production, matched as a regular expression by the sets of positions
each term may end at. Dumps print each child's field.

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
literal evaluated in it is identified by the literal and the instance (decision 28). A body with
`$` parameters and run-time ones is evaluated the same way under its instantiation, which binds
the `$` ones only; a compile-time call is the instantiation binding every parameter, so function
literals, closures and type literals are identified by the instantiation they are evaluated in
(`Func::within`, decision 362). A
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
  are at paths of one rule, `targets.<os>.<arch>.<abi>` (`targets.linux.x86_64.kernel`,
  `targets.bare.thumbv7em.eabihf`, bare metal naming an instruction set profile): the path is where
  a target is found, and code reads what it is from its fields, never from its path: the
  architecture, CPU, system and ABI, and `isa`, the
  instruction set its code is written in, which inline assembly names (spec 07, section 6):
  the architecture's own, but `thumb` for 32-bit Arm, and `features`, what its processor has of
  that set beyond what every one has, as the set's table names it (`application`, Arm's A
  profile, on `targets.linux.arm.kernel`; none on the Cortex-M targets). A `Spec` also holds the
  target's C model (`lang.target.C`: the width and alignment of each C integer type), which the C
  backend declares its types from. Adding a target adds a declaration; the compiler is not
  rebuilt. Compile-time code reads the chosen target as `lang.target.current`, the value of its
  declaration (`Declared`: the `Spec` read, and the value it was read from), and picks what
  differs between systems by its members: `std.os.libc` finds its system's row by
  `current.os`. An address is 16, 32 or 64 bits wide; any other `pointer_bits` does not read. `register_bits`
  is a general register's width, the widest integer the processor computes with (64 on wasm32,
  whose addresses are 32 bits), which native code's legalization splits wider ones by and past
  which `lang.lower.soft` computes. `floats` lists the float types the processor computes, the
  others computed by `lang.float.soft` (spec 04).
- **The target is chosen once, and nothing assumes one.** `--target` names a declaration of
  `targets` by its path (`--target bare.thumbv7em.eabihf`); without it, `targets.hosts` gives the
  target of the machine running the toolchain, named as Rust's `std::env::consts` names it. The
  choice is the query `Chosen`, and its result the input `TargetOf`, which has no default: every
  width, byte order and layout is read from it, and is unknown until it is set.
- **One binary, three kinds of component** (decision 131). `vun` is one executable, and what it
  holds is decided per component:
  - *compiled in*: the front end, checks, MIR, the interpreter, `fmt`, the language server,
    packages, diagnostic renderers and the pure-Rust backends (C text, Cranelift); enough to check,
    build and run a program with nothing else installed. Cargo features choose the set.
  - *a native library loaded on first use*: libclang for `vun ffi bind`, libLLVM for the LLVM
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
  GNU ld adapter turns them into a linker script defining the symbols at their bounds
  (`vun_link::Bound`: `__data_start`, `__data_end`, `__data_load`, `__ram_end`), and a linker that
  places nothing refuses a plan that does (`link.unsupported`). Bare-metal targets link through
  their C compiler's linker for that.
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
| wasm | WebAssembly without a C toolchain | wasm32 | the toolchain's own: relocatable objects its own linker links, section 8.4 |
| external | anything | any | a process reading LIR text |

**Choosing a backend.** A target lists the backends it builds with under each profile, most
preferred first (`lang.target.Backends`: `debug`, what `vun build` uses, and `release`, what
`vun build --release` uses): `linux.x86_64.kernel` debug builds prefer `native`, then `cranelift`, then
`c`, and release builds `llvm`, then `c`. A build takes the first that the toolchain has (its
Cargo feature built in), that generates code (`BackendInfo.readiness`) and that generates it for
the target's architecture (`BackendInfo.arches`); that is `Registry::choose`, its reasons typed
(`Unfit::{Absent, Planned, Arch}`). `--backend` (or `VUN_BACKEND`) names one instead, which must
fit; otherwise the build is refused with `backend.unavailable` or `backend.unsupported`, naming the
backends that could, and a target none of whose preferences fit reports `backend.unchosen` with
each one's reason. The profile also reaches the tools: a release build adds the target C
compiler's `optimize` flags. `vun toolchain show` lists each backend with whether it
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
undefined. It builds C as data (`vun_backend_c::ast`) and prints it once; `vun debug dump code --backend
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
  calls, and, where the compiler offers nothing for them, atomics (without its builtins,
  `lang.target.Atomics`), data aligned past every C type or placed in a link section, naked
  functions, and symbols named in inline assembly (without GNU attributes,
  `lang.target.Attributes`: `__attribute__((aligned(n), section("name")))` on the object,
  `naked` on a function taking and giving nothing whose body is plain `__asm__` alone, `used` on
  what a block names, which it writes by its ELF name) (`backend.lacks`).

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
it uses, so no function name means anything to the compiler; `-fno-omit-frame-pointer`, the chain
a backtrace walks), the flags its links take (bare
metal: `-nostdlib -nostartfiles`), the libraries of its own they name after the objects (`gcc`
where it links none by itself), its atomics, and how it reports frame sizes
(`lang.target.FrameSizes`). `vun build` writes the backend's artifacts under
`.vun/build/<content hash>/` of the root file's project, the hash of the artifacts, the C compiler
and the profile, compiles each source with it through `vun-tool`, in parallel, with
`-fstack-usage` where it has it (`vun debug dump frames` prints the sizes read back,
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
- **`vun-isa`**, **`vun-isa-x86_64`**, **`vun-isa-arm64`** (base layer, so sema checks inline
  assembly against them): what an instruction set is (labels, `Isa`, immediates, `Blocks`), and
  each architecture's instructions, operands and listings; **`vun-asm-x86_64`**,
  **`vun-asm-arm64`** encode them into buffers and hold the calling conventions' registers. The
  instructions are declared once as a table of forms: the kinds of operands a form takes (a register of some width, memory, an
  immediate of some width, a condition, a label), the widths it is defined at, and its encoding.
  The encoder emits an instruction by the first form its operands fit, the printer writes the
  same form's text, and callers ask which forms fit before choosing operands. So an instruction's
  encoding and its text cannot disagree, and a test decodes every form with an independent
  decoder (yaxpeax) and compares both. A form, or a width of it, may need what not every
  processor of its instruction set has (`lang.arch.Need`, `vun_isa::Need`: a feature, or a
  floating-point unit computing floats of a width). A buffer is made for a processor
  (`vun_isa::Processor`: the target's features, typed by the set's generated `Feature`, and its
  floats), and every assembler emits by the first form fitting the operands that the processor
  has; where it has none of them, the buffer records what the first lacks and leaves the
  instruction out, and the framework refuses the function where that instruction came from
  (`Missing::Unavailable`, `backend.lacks`). Instruction selection therefore never assumes a
  feature: the operands a snippet step brings into registers are chosen against the same
  forms, and whatever an isel emits, snippet or sequence of its own, is checked in one place.
- **`vun-object`**: relocatable objects as data, written once in the target's format: ELF of
  the class the machine's addresses fix (ELF32 for 32-bit Arm, whose objects carry EABI 5 and the
  convention's float ABI in their flags, relocations with explicit addends, Thumb functions with
  bit 0 of their value set, and `$t`/`$d` mapping symbols where code and data change), or
  Mach-O 64-bit for arm64 (`macos.arm64.darwin`), whose symbols are C names with a leading
  underscore, whose relocations keep a word's addend in place and an instruction's in an
  `ARM64_RELOC_ADDEND` before it, and whose debugging sections are `__DWARF` ones, linked by
  `ld64.lld` (lld's Mach-O flavor), all from the target as data: its `arch`, its `os` and the
  oldest release it runs on (`lang.target.Spec.os_version`, `-platform_version`), its sysroot
  (`-syslibroot`) and its C compiler's `libraries` (`System`), into a program `LC_MAIN` enters
  at `_main`; its C compiler is told the release by the option the target names
  (`lang.target.CCompiler.version_flag`); relocation
  kinds are the framework's, each format and machine numbers them. A target whose format the
  native backend does not write for its machine is refused (`backend.lacks`). ELF itself is
  `vun-elf`'s: a file as data (its class, kind and machine, sections with their contents,
  segments by what each covers, symbol and relocation tables, each machine's relocation numbers),
  each record ELF lays out declared once with its fields' order per class, from which the file
  is written, read and measured; `vun-object` maps an object onto it, and tests read the objects
  and programs of other tools through it. Each
  architecture's instruction compilers take the target's variant of the C calling convention:
  on arm64, AAPCS64 or Apple's, whose C calls pack scalars on the stack at their own size.
- **`vun-dwarf`**: debugging information (DWARF 4) as sections of an object: one compilation
  unit holding an entry per function (`DW_TAG_subprogram`: its symbol's name, the file and line
  declaring it, where its origin has one, and the range of its code), and the line table of every
  function, a row per instruction whose line changes, from the
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
- **Naked functions** (`#[abi.naked]`, taking and giving nothing): the LIR verifier allows them
  only blocks without bindings, no slot and no return, and the framework emits their blocks with
  no prologue, saves or epilogue. What any block names by symbol is reached (`Reached`), so a
  function only a block calls is kept.
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
  object, with relocations for calls, data and imports. `vun debug dump code --backend native` prints
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
every program on linux.arm64.kernel too, through the runner of the machine's tools file (spec 12,
section 6), and require the same results as on x86-64.

Thumb-2 (decision 293) holds integers and singles' bits in general registers of 32 bits, and a
double in the float bank (`Arch::bank`), which has no registers: the allocator keeps each value
of it in its home, two words, never made where used, moved between homes a word at a time; each
operation on doubles (`Selection::Homed`) loads its operands into `d0` and `d1` (`vldr`),
computes there (`vadd.f64`, `vcmp.f64`, `vcvt`) and stores its result to its home (`vstr`), or
into a general register for a single or an integer; calls and the entry pass doubles between
homes and the convention's `d` registers. A comparison of doubles sets no flags a branch fuses.
The backend tests run every program on linux.arm.kernel too, where the machine's tools file names a
runner for it.

**Legalization.** Each backend declares the widest integer it computes with
(`BackendCaps::widest`: a number of bits, or `Widest::Register`, the target's register width,
which the native backend computes with); before it reads a unit, `Legalize::apply` splits every wider
integer into limbs of the widest (low first, placed in memory by the target's byte order; an
`i128` is four limbs on a 32-bit target, decision 292): add and subtract with carries and
borrows running up the limbs, multiply by products of limbs made whole through half-width
partial products and added at their positions, shifts by any amount below the width (bits
crossing between neighbouring limbs, then whole limbs moved), comparisons by the highest limbs
that differ, extensions, truncations, selects, loads, stores, block parameters, switches, and
calls. A wide parameter is one `AbiType::Wide { limb, limbs, ext }`, its limbs values that each
backend's call classifier places together by its convention (an even register pair under
AAPCS64, two registers or 16 aligned bytes of the stack under System V, as C passes `__int128`;
decision 246); under the C convention an integer of more than two limbs is no C integer
(`Unpassable::Integer`); the C backend passes two limbs under the C convention as the C integer of
their width (`__int128` only where the target has it, on a 64-bit one) and each limb as a
parameter of its own under Vun's. A wide result is one `AbiType::Wide` too, written through the caller's
place as its limbs (returned in consecutive result registers under the C convention, as C returns
its wide integers, through memory under Vun's; decision 280). Division and remainder call a helper
function the pass adds per count of limbs (restoring division over limbs, signs put back around
it). A backend dividing by the target's processor's own instructions says how it divides there
(`Backend::division`, `Division::Computed` by default): the native backend asks the
architecture's table whether the processor has a form of its division instructions at the width
of its registers (`Native::divides`), and where it has none (`Division::Restoring`; Armv7-A
without `divide`) every division and remainder no wider than the widest calls the helper of one
limb, its operands extended to the widest as the operation reads them and the result truncated
back, so no program is refused for it. Atomic accesses of wide
integers are refused (`backend.lacks`); conversions between them and floats never reach the pass,
running as `lang.float.soft.convert`'s. A float the backend does not hold (`BackendCaps.floats`:
native code holds those the target's processor computes) is its bits, an integer of its width
and limbs where that is wide: every operation on it ran as `lang.float.soft`'s, so only moves and
bitcasts are left, and a C signature carries one only where its convention passes floats in
general registers (`Unlegalized::FloatAbi`, decision 297). A backend that
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

The toolchain's own backend for WebAssembly (`wasm32` targets, `wasi.wasm32.p1`), in one crate,
`vun-backend-wasm`. It writes a relocatable object in the format of the WebAssembly tool
conventions (a module with the `linking` and `reloc.*` custom sections), which the toolchain's
own wasm linker links in process (`vun-link-wasm`, the target's `linker`, below), so no C toolchain
is needed; its objects link with C compiled by clang for the same target, and `wasm-ld` (`lld`,
with `--linker lld`) links them alike. The encoding both write, and the objects the linker reads,
are declared once in `vun-wasm` (`compiler/object/wasm`) (proposed, decision 388).

- **Modules as data.** A module is a typed value (`Module`: types, imports, functions, data
  segments, symbols, relocations), written once into bytes; instructions are a typed set
  (`Op`), each declaring its encoding and the operand types it pops and pushes. `Module::verify`
  checks every index and runs the validation algorithm of the WebAssembly specification over
  every body, so a module the backend makes is valid before it is written; tests also validate
  with an independent parser (`wasmparser`).
- **Values.** Integers up to 32 bits and addresses are `i32`, 64-bit integers `i64`, floats
  their own (`BackendCaps::widest` is 64 bits, `f16` is promoted). A value narrower than 32 bits is
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
- **Variadic calls.** A call of a C function's variadic arguments writes them into a buffer in the
  caller's frame, each at its alignment and at least a word's, and passes its address after the
  fixed arguments: an integer narrower than a word is written as a word, an `f32` as an `f64`, a
  record or anything passed in memory as the address of a copy. `f16` has no C convention on
  wasm32 (clang refuses `_Float16`) and is refused (`Missing::Half`).
- **Sections and storage per thread.** A datum placed in a section (`#[link.section]`) is a
  segment named by it, which the linker places with the others of that name and gives bounds.
  With one thread (`lang.target.Threads.one`, WASI), storage per thread is the program's and its
  address a datum's; with many, it is refused (`Missing::PerThread`).
- **Refused.** Inline assembly (wasm32 has no table of instructions yet), naked functions (which
  exist only for it) and an address past the start of a function (a function's address is an
  index of the table) are refused with the `Missing` naming each.

**The wasm linker.** `vun-link-wasm` reads the objects in the tool conventions' format (the wasm
backend's, clang's) and writes the program, as `wasm-ld` does:

- **Symbols.** A named symbol has one definition: a strong one over a weak one, two strong ones
  refused (`link.duplicate`); a COMDAT group is kept from the first object declaring it and
  dropped from the others. An undefined function of a library stays an import of the program; one
  no object defines and no library provides is refused (`link.undefined`), unless weak: calling
  it traps (a stub per type, `undefined_weak`) and its address is null, as a weak datum's is. The
  linker defines `__stack_pointer`, `__indirect_function_table`, `__wasm_call_ctors` (calling each
  object's initialization functions by priority), `__data_end`, `__heap_base`, `__dso_handle`,
  `__stack_low`, `__stack_high` and, for each section a source names, `__start_<name>` and
  `__stop_<name>` (the tool conventions') and `__<name>_start` and `__<name>_end` (the link
  plan's, `vun_link::Bound`).
- **What is kept.** The entry, the functions objects export (under the name an object's export
  gives one, else the symbol's) and the symbols objects keep (`NO_STRIP`) are the roots; a
  function, segment or import nothing reaches from them through relocations is left out.
- **Memory.** The stack comes first, 64 KiB growing down from its top, so overflowing it leaves
  memory rather than writing over data; then `.rodata`, `.data`, the sections sources name, and
  `.bss`, each segment at its alignment. Each section is one data segment, and one holding only
  zeros is not written, memory starting as zeros. Memory is exported as `memory`.
- **Relocations** are applied in place, as the objects padded their fields; a function whose
  address a relocation takes gets a slot of the table from the second on, the first null.
- **Names.** A `name` section names every function, for the traces of a runtime.
- **Refused** (`link.malformed`): what is not a relocatable object of the conventions, and what
  the linker does not link: tables, memories or globals an object defines, passive segments,
  storage per thread of shared memory, and relocations other than of function, table, type,
  global and table-number indices and of memory addresses (the PIC and TLS kinds). An object's
  element and data-count sections, its view of the table, are read past, the table being made
  from the relocations.

### 8.5 Calling conventions

Where a call passes each value is decided once, in `vun-abi`, from the convention as data, and
every backend that places arguments itself reads the result; no backend states a convention's
rules (proposed, decision 256).

- **Conventions as values.** A `Convention` is its `Rules` and the machine's registers they name:
  the argument and result registers of each class (general, floating-point); where the address
  of a result in memory goes (the first general argument register, System V and AAPCS, or a
  register of its own, `x8` under AAPCS64). The rules declare: the word (the bytes of a general
  register, an address and a stack slot) and the stack's alignment at a call; how records
  travel (`Records`: System V's eightbytes, each general or SSE by its fields; or homogeneous
  aggregates of up to four floats of one type, a field per floating-point register whatever the
  record's size, and other records a word per general register); the largest record passed in
  registers and the largest returned in them (16 bytes under System V and AAPCS64; any size
  passed and a word returned under AAPCS); what becomes of a larger record (copied onto the
  stack whole, or passed as the address of a copy the caller makes); whether a value that finds
  too few registers of its class leaves the rest of the class unused (AAPCS64, rules C.11 and
  C.13; AAPCS, C.2 and C.6) or leaves them to later arguments (System V); where a value aligned
  to two words starts in general registers (at the next, as System V and Apple's variant take
  an integer of two words, or at the next even one, AAPCS64 rule C.9 and AAPCS rule C.3, which
  also applies to records so aligned); where floats travel (`Floats`: each in the next
  floating-point register, System V and AAPCS64; in general registers, AAPCS's base standard,
  the soft-float EABI; or in single-precision registers a double taking an aligned pair named by
  its first, each value in the lowest free run so that a float fills back a register a double's
  alignment skipped, AAPCS's VFP variant, rule C.1); whether a record finding too few general
  registers may take the rest of them and the stack (`Split`: never, or while nothing is on the
  stack yet, AAPCS rule C.5); the stack (a slot of a word, scalars in the words they take at
  their own alignment, or scalars at their own size and alignment, as Apple's variant packs a
  C call's; composites aligned to at most the stack's alignment); and how a call passes a C
  function's variadic arguments (`Variadic`: as fixed ones, AAPCS64; as fixed ones counting the
  floating-point registers used, System V's `al`; each on the stack in the words it takes
  whatever registers are left, Apple's variant, so that `open`'s mode is the first stack slot;
  or everything the call passes and returns as the base standard does, floats in general
  registers, AAPCS's VFP variant, section 6.5). A LIR signature marks where its variadic
  arguments start (`Signature::variadic`), which only an import under the C convention has;
  `Rules::calling` gives the rules a call of it follows, legalization asking them too. The C
  backend declares such an import with `...`; the wasm backend, whose C convention passes them
  in a buffer the caller fills, refuses it (`Missing::Variadic`). `SYSV`, `AAPCS64`,
  `DARWIN_ARM64`, `AAPCS` and `AAPCS_VFP` are the five built; riscv64's LP64D and wasm32's C
  convention are further values of the same type.
- **A value in one register.** A LIR value never spans registers: a scalar wider than a register
  of its class (a 128-bit integer, or a 64-bit one or a soft-float `f64` on a 32-bit machine) is
  refused (`Unpassable::Wider`), legalization having split it into limbs first by asking the
  rules (`Rules::fits`); its limbs are one `AbiType::Wide`, placed as the rules place a value
  of two words, or one limb per register under Vun's convention (decisions 280, 292).
- **One classifier.** `Passing::of(signature, convention)` gives every LIR value of a signature
  (`Signature::passed`: a wide integer's limbs each, the address of a result in memory last)
  a typed placement: a register, a place on the stack, a record's parts in registers with their
  offsets and sizes and the rest of it, if any, on the stack (`Loc::Split { parts, rest }`), a
  record copied onto the stack, the address of a copy the caller makes and
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
  `lang.target.Convention` (`sysv`, `aapcs64`, `darwin_arm64`, `lp64d`, `aapcs_vfp`, `aapcs`,
  `ilp32`, `wasm32`), so no backend matches a target's ABI by name; an architecture lists the conventions
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
frame accesses to 264); the LCG loop of decision 230 runs its 50 million rounds in 5
instructions each instead of 13 (7 before its two 64-bit constants were held across the loop),
in registers a call may overwrite, none saved.

- **Linear scan on SSA**, after Wimmer and Franz (2010): LIR is in SSA form with block
  parameters, so each value's lifetime is built in one backward walk of the blocks in `Order`,
  as intervals of positions with holes, and its uses with what each needs. Intervals are
  allocated in order of their start. A value that finds no register free for its whole
  lifetime is split: the part up to where the register is next needed keeps it, the rest waits
  for another, and a part that finds none is spilled to a home in the frame, split again before
  a use that needs a register. Values made anywhere (constants, addresses known while
  compiling, section 8.3) that the architecture's instructions take in place, as an immediate
  or an address operand, are made where used and have no interval. Another is held in a
  register (`Remade::Held`) where a use is in a loop its definition is outside of, as a constant
  hoisted out of a loop is (section 3), and made where used otherwise: held, it has no home,
  so a spill stores nothing and a reload makes it again, and a register only such values hold
  is taken first where they need it later.
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
| wasm | the toolchain's own, in process: WebAssembly objects of the tool conventions; WASI's (section 8.4) |
| GNU ld | vendor toolchain compatibility |
| none | objects or static libraries for foreign builds |

### 8.7 Checks of unsafe code

- **Where.** Rewriting calls happens when a function is made concrete, in the same way as
  `#[lang.lower.by]`: the query `Concrete` reads the program's checks, and a call of something
  marked `check.by` for a checked kind, or of a function implementing a requirement so marked
  (`Func::requirement`, the query `Requirement`), becomes a call of its checked version. MIR still has the
  types and the calls there. LIR has only addresses and could not tell a raw pointer from a
  borrow the checks already proved. Everything else stays where it lives today:
  - Under the `pointers` or the `asm` checks, lowering (`vun-lower`) gives the slot of a local
    whose address is taken (borrowed itself, not through a borrow it holds) a start at a shadow
    granule (`lang.check.granule`) and
    `lang.check.redzone` bytes after it. At entry the slot is marked accessible as far as it
    reaches and its redzone poisoned; at each return both are marked accessible again, since
    the stack past a frame is the next call's. The checks do not apply in the functions they
    run in: each function the runtime exports for an operation of `lang.check`, and every
    function one calls, or an operation would check itself.
  - Under the `asm` checks (`vun_lir::Unit.checks`), lowering gives each assembly block its
    check (`InstKind::Asm`'s `AsmCheck`): a site of its own, and the runtime's helpers machine
    code calls with scalars only, under the C convention, `lang.check.asm_write(address, bytes,
    site)` and `asm_stack(site, before, after)`. The native backend calls `asm_write` before
    each instruction whose form writes memory, with the address that operand computes, and
    `asm_stack` where a block that returns moved the stack pointer. The C backend checks the
    stack pointer only, which needs no encoder.
  Sema holds the checks (`vun_sema::checks::ChecksOf`, an input as the target is) and what an
  operation runs as under them (`Attributes::checked`); `vun-sanitize` holds the link symbols of
  the operations a runtime provides (`Operations`), which the driver reads to report
  `check.unsupported`.
- **Runtime state is shadow memory**, one byte per 8 bytes of address space, as AddressSanitizer
  encodes it: 0 for eight addressable bytes, `k` for the first `k`, and a negative value per
  reason (`lang.check.Violation`) otherwise. `std.check` holds the shadow's logic, reading and
  writing it by atomic accesses, which no check rewrites, and the exports a runtime makes of
  it (`std.check.exports`). The shadow of each span of 2^32 addresses is reserved the first
  time memory in it is described, backed only where touched, and a directory of a word per span
  says where each is (0 where none is yet, which reads as accessible), so the shadow follows the
  program's memory wherever the system puts it, whatever width of addresses the system gives
  programs; a runtime that maps memory gives `std.check.mapped` how it maps and gives back memory
  (`mmap`), and the shadow is reached through `lang.mem.pointer_at`. A board runtime is to take it from a `.shadow` region of
  `Options.memory`, one eighth of the RAM it checks, and a WASI runtime to keep it in linear
  memory, grown with the memory the program uses (`memory.grow` by an eighth); until they do,
  a checked build for them stops at `check.unsupported`. `asm` does not arise on wasm. The
  redzone size and the quarantine's bound are named data in `lang.check` (`redzone`,
  `quarantine`), read by lowering and the runtime.
- **The rest of the state** lies in words past the shadow (`std.check.State`), taken under a
  lock word: a table of what allocators have handed out, an address and its bytes per entry,
  open-addressed by linear probing with entries moved back on removal (`std.check.table`); and
  the quarantine, a ring of memory given back (`lang.check.Held`: the address, bytes, alignment
  and the allocator's address), oldest first (`std.check.quarantine`). A runtime that maps memory
  has it all from `std.check.mapped($map, $unmap)`, which maps the directory and the state once,
  and each span's shadow when needed, the first thread to need either winning. The checked `free`
  (`std.check.free`) asks `lang.check.freed`, which fails with `foreign_free` for an address the
  table does not hold (`double_free` when its shadow says it was given back), `size_mismatch`
  for other bytes than the table records, and otherwise removes it and poisons the bytes. It then
  holds them in the quarantine (`hold`) instead of giving them to the allocator, and gives the
  allocator what `evict` hands back: while the quarantine holds more than `lang.check.quarantine`
  bytes or has no room, the oldest memory of the allocator freeing. Memory leaves the quarantine
  only for the allocator that is giving memory back at that moment, which is alive, never for
  one that may have been dropped since; the memory of an allocator that never frees again stays
  there, and a full ring gives memory back at once.
- **Why not fat pointers.** Carrying bounds in `*T` would change its size, and with it every
  layout holding one, `#[layout.c]` and the C ABI. A checked program could then no longer call
  C, or a library built unchecked. Shadow memory keeps every layout. Code built without checks
  still links, and its allocations are registered, because its allocators run their checked
  versions.
- **Known gaps, stated rather than hidden.** Data objects are addressable as wholes: an overflow
  from one datum into the next is not caught, since that would need the linker to lay out
  redzones. Accesses through borrows are not checked, because the borrow check proved them.
  An `asm` block reading memory it does not own is not caught, only writes are. A slot is
  not poisoned once its function returns, so a pointer to it used afterwards is caught only
  where another call's redzone lies there; catching it always would take frames on a stack of
  their own (`Violation.returned` is kept for that). The table of allocations is
  bounded (`std.check.table.entries`): past it, allocations are not recorded, and giving them back
  reports `foreign_free`. An allocator whose `free` gives memory to another allocator it wraps is
  checked twice, so the second check reports memory given back twice.

## 9. Workspace

```
compiler/
  base/    index  span  intern  origin  diag  query  pass  tool  target  trace  json  codec
           codec-derive  hash  graph  fs  outline  par  suggest  term  bytes  harness  html
  lang/    grammar  items  primitives            Rust views generated from lib/lang
  isa/     api  inline  x86_64  arm64  thumb     instruction sets, one table each
  object/  core  elf  dwarf  wasm                object files and debug information as data
  front/   desugar  syntax  hir
  sema/    int  float  ty  core  plan            types as values; `core` (`vun-sema`) checks and evaluates
  mid/     mir  passes  interp  sanitize
  low/     lir  lower  abi  passes
  back/    api  codegen  regalloc  c  wasm  cranelift  qbe  llvm  external
  asm/     core  x86_64  arm64  thumb            assemblers over the isa tables
  native/  core  x86_64  arm64  thumb  backend   the native backend, an architecture per crate
  link/    api  wild  mold  lld  wasm  gnu  msvc  ar  external
  render/  short  annotate  ariadne  json  sarif
  pkg/     build  fetch  http                    manifests, locks, the store, fetching
  tools/   fmt  reduce  bind  clang  command  doc  driver  lsp  cli  generate  xtask
fuzz/      cargo-fuzz targets over the verifiers, a workspace member
lib/
  lang/     primitives (declarations with `= _`), syntax/grammar.vun and syntax/desugar.vun
  std/      prelude, option, result, cmp, int, iter, seq, ptr, task, mem, sync, fmt, text, meta,
            diag, lints, log, hw, os, simd, panic, testing, build, collections
  targets/  a namespace per system, a target per CPU: targets.<system>.<cpu>
  boards/   example runtimes: stm32f4, rp2040, esp32c3
```

Directory names are short because the path gives context; packages are named `vun-<name>`
(`compiler/back/c` is `vun-backend-c`, `compiler/back/api` is `vun-backend`). A family of crates
sharing a concern is a group of its own (`isa/`, `asm/`), its shared crate named `api` or `core`,
so that no two directories share a name within a group and every crate sits at
`compiler/<group>/<crate>`.

Rules:
- `vun-driver` is a library; the CLI, the LSP and the test harness are thin shells over it.
- A crate depends only on crates in its own or earlier groups: `base` < `lang` < `isa`, `object` < `front` <
  `sema` < `mid` < `low` < `asm` < `back`, `link`, `render` < `native`, `pkg` < `tools`; the order is
  declared once in `xtask/src/layers.rs` and checked by `cargo xtask layers`. Where a later stage implements something an earlier one
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
  query's name and its own work (verifying memos and verifying them ahead, reading saved keys,
  memos and values), and the driver each stage of its table and each build step (the program's
  reach and its bodies' reports, lowering, legalizing, emitting). Each activity sums
  its runs, its own time (without the activities run inside it) and its total time over every
  thread. `VUN_TIMINGS=1` or `--timings` reports the table at exit, the most own time first,
  with how work spread over threads (calls spread, run alone or nested in a worker, helpers
  asked and lent, wall and busy time); when off, timing an activity costs one relaxed load.
  Waiting for another thread's memo is an activity of its own. The report gives each
  activity's longest run too, and `VUN_TIMELINE=<file>` or `--timeline <file>` writes every run,
  on every thread, as a timeline Perfetto opens (Trace Event Format): what overlapped, what a
  thread waited on, and the longest chain of work. `VUN_THREADS=n` sets the threads
  a call runs on. `VUN_MEMOS=1` or `--memos` reports what each query did with its memos:
  restored, keys and values read, found to hold in order or ahead, and each computation by why
  it ran; counts, the same every run on one thread.
- There is one frontend: the LSP runs on the same query engine as the compiler.
- Command lines are declared, never matched by hand: `vun-command` holds command trees (names,
  aliases, typed parameters with their values and variables, shared option sets, examples,
  subcommands, actions) as data, and parses, validates, renders help and completion scripts, and
  reports usage errors as diagnostics with suggestions, from them. Options declared on a group
  or in a set apply to every command below it (`vun debug --diagnostics short cst f.vun` and `vun
  debug dump cst f.vun --diagnostics short` are one request), and a program's `configure` step applies
  them before the command runs. `vun-driver` declares the `vun` tree; `xtask` and `generate`
  declare their own with the same crate, and each binary's `main` is `Program::main`.
- Terminal output goes through `vun-term`: text is built from pieces naming their `Style`, one
  palette maps styles to colors, tables align columns, and a `Shell` holds standard output
  (results) and standard error (status lines, diagnostics), each colored as `--color` decides.

## 10. Test case reduction

A file that makes the compiler fail is cut down to the least text that fails the same way
(`vun-reduce`, `vun debug reduce <file>`). A failure is typed: the compiler crashing at a place of
its own source, or the file reporting an error id; `--keep <id>` names the error to keep, and
otherwise the file's first failure is kept. Each candidate is compiled in the driver's process as
the file it stands for, through every check and the chosen backend's code generation, over the
same queries, so the library is compiled once.

Edits follow the concrete syntax tree, as in hierarchical delta debugging: runs of a node's
children are removed, the whole run first and then halves down to single children; then a node is
replaced by one of its children; then single lexemes are removed. Each pass runs until none of its
edits is kept, and the passes repeat until none is. A candidate that parses with more errors than
the text it came from is dropped before it is compiled, and no text is compiled twice. The result
is laid out by the formatter when that keeps the failure. It is what a fuzz target's crash becomes
before it is added to the corpus.

## 11. The language server

`vun util lsp` (`vun-lsp`) serves an editor over standard input and output with the Language
Server Protocol: JSON-RPC messages framed by `Content-Length` headers. It is a thin shell over
`vun-driver`: one session, whose query engine is the compiler's, so what the editor shows is what
`vun check` reports, computed by the same queries, and each answer reuses what the last computed.
A thread reads the messages as they come, so that the loop handles every one waiting before it
checks what changed; where no thread starts (WASI's first preview, `vun_par::SPAWNS`), the loop
reads each message itself once the last is handled, and checks after each.

- *Documents.* The text the editor holds of an open file is the file's text, whether saved or not:
  opening it and each change (the whole text) read it as a new version of the file, a file of its
  own (`FileId`) under the file's key, which the `FileText` input is set to; a file not open is
  read from disk, and closing one reads it from disk again. What a query computed from an earlier
  version is shown in the latest (`vun_origin::Resolve`): each origin is anchored to its
  declaration, so what an edit leaves alone keeps its results, placed where the new text holds
  them, and only what depends on the edit is computed again.
- *Diagnostics.* After a change, the server checks the open files as `vun check` checks each, and
  publishes each file's diagnostics with the version of the document they were computed for,
  mapped as spec 10, section 6 says. A check runs to its end; the changes that arrive meanwhile
  are applied together before the next, so a burst of typing costs one check. Changes arrive as
  edits of ranges (incremental sync), applied in order to the document's text.
- *Definition.* At a name, where it is declared: the binder of the file it resolves to, or the
  declaration an outer name, a member or a path reaches, as resolution and sema find them.
- *Hover.* At a declaration's own name, the declaration; elsewhere, the innermost expression
  holding the position: `name: T = value`, its name when it is one, its type as `vun debug dump
  types` prints it, and its value when compiling computes one other than a function or type
  literal it writes (the value of the declaration it names, or else its own, `vun_sema::value::
  ExprValue`, evaluated where it is written); its stage; the layout of the type it is, or else of
  its type, as `vun debug dump layout` prints it; and the doc comment of the declaration.
- *Outline.* The declarations of the file, read from the syntax tree alone, so that a file that
  does not check has one: each binding by what its value is (a function, a struct with its
  fields, an enum with its variants and the members of its namespace, a constant or a variable).
- *Folds.* Each pair of delimiters, each run of raw string lines and each run of comments on
  consecutive lines, when it spans several lines.
- *Formatting.* The one edit making the document what `vun fmt` would write, from the first byte
  that differs to the last; none for a file that does not parse.
- *Semantic tokens.* Every token classed for coloring: comments, literals, keywords and operators
  by their kind and the grammar's tables; contextual words (borrow modes, flows, `in`, `asm`,
  mnemonics) by the node they anchor; a name where it is declared by what its binder declares, and
  where it is used by what it resolves to: a parameter, a local, a member of a struct, a variant,
  or a declaration classed by the value sema computes for it (a type, a function, a namespace, a
  constant). Modifiers say a declaration, `mut`, `$` and documentation.
- *References and rename.* At a name, or at the binder declaring it, the declaration it names
  and every place naming it: its binder, and each name, member and path that resolution and sema
  lead to it, matched by declaration and name together (one pattern declares several names). A
  local is searched in its file; a declaration in the files of its package and of the package
  asking, in parallel. Renaming edits each of those places, the new name spelled as the source
  writes it (escaped when it is a keyword); it is refused when the new text is not a name, when
  nothing is named at the position, for a namespace (named by its file) and for a declaration of
  another package. Edits name the version of each document the client holds.
- *Code actions.* The fixes of the diagnostics over the range asked about, each a quick fix
  applying its edits, except those only shown and those whose edits are in an earlier text of
  their file; and each view of the document (`vun_driver::view::View`, the representations `vun
  debug` prints: those of any file, of a package's `build.vun`, of the program a root file
  exports, as the file is one), a `source` action running the command `vun.view`.
- *Commands.* `workspace/executeCommand` carries out the commands the server announces, one row
  each. `vun.view` writes a view of a document in its project's `.vun/views/<path>.<view>`, at the document's path in the project,, and asks
  the client to show it (`window/showDocument`); what computing it reports is left to the
  document's diagnostics. `vun.run` builds the program the document exports and runs it, without
  input, its output written to `.vun/views/<path>.run` as it comes, then how it ended; what stops
  the build is written there instead. `vun.test` builds the tests' program of the document's
  namespace (spec 11) and runs one test by name, what `vun test` reports of it written to
  `.vun/views/<path>.test` as it comes. When the run ends, how each test ended is kept until its
  document changes: its lens's title says it (`Test: trapped`), a trap is published as an error
  where it fired, in whichever open document that is, and the client is asked to refresh its
  lenses (`workspace/codeLens/refresh`). The server's loop handles events: the messages the
  client sends and what the tasks it started report.
- *Code lenses.* Over the name of each declaration of a root file exporting its program
  (`vun_mir::Program::roots`), a lens running `vun.run`; over the name of each test the file
  declares, a lens running `vun.test`. The tests are those the runner finds, `std.meta.collect`
  of the file's namespace called through the query engine (`vun_sema::testing::Tests`), never a
  match of `#[test]` in the text.
- *Signature help.* In the arguments of a call, its callee's declaration as the source writes it
  (its name, parameters and result, from a function literal or else a function type), each
  parameter, the one the argument at the position fills, and its doc comment.
- *Inlay hints.* After the name of each binding without a type, the type of its value, as hover
  shows it; none for a value that is a function, a struct or an enum, which says what it is.
  Layouts are shown on hover only.
- *Completion.* After `.`, the members of what is before it: a namespace's (all of them in the
  asking package, else the public), or the fields, declarations and variants of the struct or
  enum it is or has the type of, through a pointer. Elsewhere, the names in scope: the parameters
  and locals of the function holding the position declared before it, the file's declarations,
  and the names the package's files see from outside them. Each is classed as semantic tokens
  class it.
- *Positions* are counted in the encoding the client and the server agree on at initialization:
  UTF-8 when the client offers it, else UTF-32, else UTF-16, converted through each version's
  line index.
- *The protocol is data.* The protocol's values are typed structs and enums (`vun-lsp-protocol`),
  read and written through `vun-data`'s derives, and each method is a type naming its parameters
  and result (`requests::Hover`, `notifications::DidOpen`). The server's methods are rows of one
  table, each a method's type, when in the lifecycle it is answered and the capability announcing
  it; a handler is the server's `Answers<R>` or `Hears<N>` for that type, and the capabilities
  announced at initialization are derived from the rows. An unknown method is answered
  `MethodNotFound`, a malformed message `ParseError` or `InvalidParams` with where the parameters
  did not fit, never with a crash; logs go to standard error through `vun-trace`. The lifecycle is
  the protocol's: `initialize`, `initialized`, the session, `shutdown`, then `exit`, whose status
  is 1 without a `shutdown` before it.
