#import "../template.typ": *
#import "../figures.typ": pipeline, code-size

= Implementation <sec-implementation>

== Pipeline

The compiler is a set of Rust libraries with narrow interfaces (@fig-pipeline). Source is parsed
into a token array and a postorder syntax-tree array; desugared into a high-level representation
(HIR) with names resolved; analyzed on demand, with type checking and compile-time evaluation
interleaved because types are values; lowered to a mid-level representation (MIR), a control-flow
graph over places, on which all the checks of @sec-memory run; and lowered again to a low-level
representation (LIR), a type-erased static-single-assignment form shared by every backend. Nothing
above LIR knows about any backend, and nothing above the linker interface knows about any linker.

#figure(
  pipeline,
  caption: [Compiler pipeline. Solid stages are implemented; dashed stages are defined as interfaces
    only. The interpreter runs MIR and serves as the reference semantics.],
) <fig-pipeline>

Three decisions shape the architecture.

*The reference semantics is executable.* An interpreter of MIR under an explicit target model
(layout, endianness, ranges, traps, pointer provenance) runs compile-time code and tests, and
every backend is to be checked against it by differential testing on the corpus and on randomly
generated programs @csmith. This is what keeps guarantees 3 and 4 true across backends.

*Every stage after the front end is replaceable.* Backends, linkers, external tools, passes,
evaluators and diagnostic renderers are interfaces. Each component declares its facts once
(supported architectures, capabilities such as tail calls or frame-size reporting), and support
checks derive from them; a program needing a capability a backend lacks is rejected with a
diagnostic naming the backends that have it, never compiled with a silent fallback. Targets are
data written in Vun, so adding a target adds a file rather than rebuilding the compiler. External
backends and linkers are processes that exchange LIR text and an abstract link plan.

*Speed is structural.* Measurements elsewhere show that once a backend is fast, the front end
dominates: in TPDE, a backend 8 to 24 times faster than LLVM at `-O0` improved end-to-end
compilation by only 17%, because the backend accounted for 15% of the time @tpde. The front end
is therefore designed for speed from the start. Syntax trees are postorder arrays in the style of
Carbon @carbon-parse, and the other representations are struct-of-arrays arenas addressed by
32-bit handles, a layout that reduced Zig's front-end time by 20 to 40 percent @zig-devlog. Names,
types and compile-time values live in one sharded intern pool; a symbol is equal by pointer,
hashes by the content hash of its text and orders by its text, so results do not depend on the
process or the schedule.

== Incremental queries

The unit of incremental work is a declaration or a function body. Each query result has an
interface hash, and a memo keeps a _verifying trace_ in the sense of Mokhov et al.
@build-systems: the interface hash of each query it read. In a later revision the memo is reused
when every dependency, brought up to date, hashes as recorded, and it is checked at most once per
revision. Because traces hold content hashes rather than revision numbers, they remain valid in
the next process, which is what allows the query database to be persisted to disk. A loaded memo
is verified before its first use like any memo of an earlier revision, so a stale cache costs
recomputation, never a wrong result. A query that reaches itself receives the value given by its
cycle rule, together with the path of queries, rather than looping.

== One grammar, many consumers

The grammar is defined once, as Vun data in `lang`, together with the desugaring tables and the
list of diagnostic identifiers. A generator reads these files with the compiler's own literal
reader, which parses Vun values without evaluating them, and produces the parser's token and node
kinds, the typed accessors of every node, the parse tables, the diagnostic identifiers, and the
enumerations of primitives. A node kind declares its children as tagged slots, and the syntax-tree
verifier checks that every node fits its kind. A test fails when a generated file is stale, so the
compiler and the language's own description of itself cannot drift apart.

== Current state

@tab-state summarizes the implementation as of this draft. The workspace comprises 56 crates and
about 37,000 lines of Rust, layered so that a crate depends only on earlier groups:
`base` < `front` < `mid` < `low` < `back`, `link`, `render` < `tools`. @fig-code-size shows their
distribution. The Vun libraries amount to about 1,200 lines in `lang` and 900 in `std`,
including the compiler's tables.

#figure(
  table(
    columns: (auto, 1fr),
    toprule,
    [Component], [State],
    midrule,
    [syntax], [lexer, error-recovering parser, postorder syntax tree with a verifier, literal reader; fuzzed],
    [HIR], [desugaring through the `lang` tables; name resolution, visibility, spreads],
    [semantic analysis], [types as values, compile-time evaluation, implementations and witnesses, reflection, code values, closures, coroutines, layout, kinds, inference of `$` parameters],
    [MIR], [lowering of 83 of the 94 functions in the accepted corpus],
    [MIR checks], [place states, moves and leaks, `defer`, borrow conflicts and escapes, free values, `panic.never`],
    [interpreter], [runs compile-time code on MIR, including coroutine frames; no memory model for `&mut` yet],
    [packages], [manifests, BLAKE3 tree hashes, store and collection, `.tar.zst` archives, lockfile, minimal version selection, replacement, indexes],
    [diagnostics], [84 identifiers in 23 groups, each with a checked explanation page; renderers in the annotate-snippets, ariadne and one-line styles],
    [LIR, backends, linkers], [LIR with a verifier and the analyses backends share; lowering from MIR, coroutine frames included; a C backend writing strict C99 and a single-pass x86-64 backend writing ELF objects, which run the backend tests with identical results; linking through GNU ld],
    midrule,
    [corpus], [36 source files to accept and 58 to reject; the harness reports 207 passing checks and no failures],
    [test suite], [265 tests, including a check that every `lang` and `std` file is accepted],
    [coverage], [82% of expressions in accepted function bodies are typed],
    bottomrule,
  ),
  caption: [State of the implementation, September 2026.],
) <tab-state>

#figure(
  code-size,
  caption: [Lines of Rust per workspace group, including tests, excluding generated code.],
) <fig-code-size>

The main gaps, in the order in which they are planned, are: the remaining library surface (a thread
pool, the simulator, text and collections); the `fixed` and `scoped` kinds and the ending of
suspended frames; the driver side of the build function; the remaining MIR lowering (listed by the
corpus harness); exhaustiveness checking; an evaluator with its own
stack and a memory model for mutable borrows at compile time; and persistence of the front-end
queries. @tab-roadmap gives the phases of the project and their exit criteria; the implementation
is between phases 1 and 2.

#figure(
  table(
    columns: (auto, 1.2fr, 1fr),
    toprule,
    [Phase], [Deliverable], [Exit criterion],
    midrule,
    [0], [specification, grammar, corpus written before the compiler], [every corpus program is explained by the concepts and the desugaring tables],
    [1], [syntax, HIR, semantic analysis, interpreter, queries, origins, diagnostics; `lang` and a minimal `std`], [the corpus type-checks; the four layers of generation run],
    [2], [MIR checks, coroutine lowering, rejection corpus], [every rejection file produces exactly its expected diagnostics],
    [3], [LIR, C backend, linkers, emulator runner, test runner], [a program runs on a real board; differential tests pass],
    [4], [Cranelift and QBE backends, hosted `std.os`, benchmarks], [the test suite passes natively on three backends],
    [5], [board runtimes, executors, stack and trap analyses, deferred logging, C bindings], [measured stack use within computed bounds],
    [6], [formatter, language server, fixes, lint interface, documentation, LLVM backend], [code size within 10% of C on a reference firmware],
    bottomrule,
  ),
  caption: [Phases of the project and their exit criteria.],
) <tab-roadmap>

The planned backends are C, for reach, including vendor toolchains; Cranelift, for fast debug
builds, which reduced code-generation time by about 20% when used for rustc at the cost of code
about 14% slower @cranelift-rustc; QBE, for simplicity @qbe; and LLVM, for optimized code. A
single-pass backend in the style of TPDE @tpde may follow. The planned linkers are wild, mold, lld,
GNU ld and the MSVC linker, behind one interface that receives a target-independent plan of
objects, sections, memory regions and symbols @wild-mold.
