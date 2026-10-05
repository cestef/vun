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
  caption: [Compiler pipeline, with the implemented backends and linkers. The interpreter runs MIR
    at compile time.],
) <fig-pipeline>

Three decisions shape the architecture.

*Compile time is executable.* An interpreter of MIR, lowered once to a register bytecode, runs
compile-time code under the target's layout, word size and endianness. Run-time code is checked by
agreement instead: `vun debug compare` builds a program through every backend that generates code,
runs each result and reports whether they agree, and the driver's tests link the objects of the
WebAssembly backend with two linkers and require the same results. This is what keeps guarantees 3
and 4 true across backends. Differential testing against the interpreter on randomly generated
programs @csmith is planned.

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
hashes by a fixed-seed hash of its text, stable within one toolchain build, and orders by its
text, so results do not depend on the process or the schedule.

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

@tab-state summarizes the implementation as of this draft. The workspace comprises 106 crates and
about 152,000 lines of Rust excluding generated code, layered so that a crate depends only on
earlier groups: `base` < `lang` < `isa`, `object` < `front` < `sema` < `mid` < `low` < `asm` <
`back`, `link`, `render` < `native`, `pkg` < `tools`.
@fig-code-size shows their distribution. The Vun libraries amount to about 6,900 lines in `lang`,
9,100 in `std`, 1,400 in board packages and 500 in target descriptions, including the compiler's tables.

#figure(
  table(
    columns: (auto, 1fr),
    toprule,
    [Component], [State],
    midrule,
    [syntax], [lexer, error-recovering parser, postorder syntax tree with a verifier, literal reader, formatter; fuzzed],
    [HIR], [desugaring through the `lang` tables; name resolution, visibility, spreads],
    [semantic analysis], [types as values, compile-time evaluation, implementations and witnesses, reflection, code values, closures, coroutines, layout, kinds, narrowing, exhaustiveness, inference of `$` parameters],
    [MIR], [lowering of every run-time function the accepted corpus reaches (3,412 of 3,412); structural verifier],
    [MIR checks], [place states, moves and leaks, `defer`, borrow conflicts and escapes, free values, `panic.never`],
    [interpreter], [runs compile-time code on MIR, lowered once to register bytecode, on a stack of its own, including coroutine frames and loans for `&mut`; it runs nothing else],
    [LIR], [static single assignment with block parameters and a verifier; the analyses backends share; legalization of wide integers and `f16`; promotion, folding, value numbering, jump threading, narrowing and inlining],
    [inline assembly], [blocks checked at compile time against the instruction tables of `lang.arch` for x86-64, arm64 and Thumb-2; encoded by the native backend, written as GNU `__asm__` by the C backend, refused by the WebAssembly backend],
    [backends], [C99; a single-pass native backend for x86-64, arm64 and Thumb-2 writing ELF, PE and Mach-O objects with DWARF line tables and unwind information, over a shared linear-scan register allocator; WebAssembly objects for WASI],
    [linkers], [GNU ld, mold and wild through the target's C compiler; lld, including its COFF flavor; an in-process WebAssembly linker; static and shared libraries],
    [packages], [manifests, BLAKE3 tree hashes, store and collection, `.tar.zst` archives, lockfile, minimal version selection, replacement, indexes over HTTPS; toolchains installed from signed releases],
    [diagnostics], [137 identifiers in 32 groups, each with a checked explanation page; renderers in the annotate-snippets, ariadne, one-line and JSON styles],
    [editors], [a language server over one driver session and its query engine; tree-sitter, Helix, Neovim, Zed and VS Code packages generated from the grammar; a browser playground running the compiler under WASI],
    [tests], [a runner for unit, property, fuzz, benchmark and simulation tests, run natively on threads of their own],
    midrule,
    [corpus], [80 source files to accept and 104 to reject; the harness reports 340 passing programs and no failures, with 2 waiting for library surface; 813 tests in the compiler's own suite],
    [backend tests], [every test program is built through each backend and run on Linux for x86-64, arm64 and 32-bit Arm (through an emulator) and under WASI, with identical results required; Windows programs built through the C backend run under Wine],
    bottomrule,
  ),
  caption: [State of the implementation, October 2026.],
) <tab-state>

#figure(
  code-size,
  caption: [Lines of Rust per workspace group, including tests, excluding generated code.],
) <fig-code-size>

The native backend follows TPDE @tpde: one pass over LIR, instructions selected from snippets
declared as data, and every instruction of each architecture declared once in a table that the
encoder, the printer and instruction selection read. A loop of 50 million steps of a linear
congruential generator runs in 56 ms through it, against 51 ms for the C backend compiled with
`-O2`. This is one small program and says little about larger ones.

A Pico 2 W (RP2350) board package starts, initializes memory, reports faults and returns to its boot
ROM on hardware; its timer, interrupts and a serial link are not working yet, and bare targets
still go through the C backend by default. The main gaps are: the remaining library surface (a
thread pool, a simulated network and disk, text and collections); the `fixed` kind; the native
backend for Windows; deferred logging; and, in the native backend, running on macOS (it links, but
nothing here runs it) and support for RISC-V. @tab-roadmap gives the phases of the project and their
exit criteria. The implementation has passed the exit criteria of phases 2 and 3, the latter on a
board; phases 4 to 6 are partly done.

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

Backends and linkers are chosen per target and profile from lists the target declares, among
those the machine's toolchain provides. Cranelift, QBE and LLVM backends and the MSVC linker are
declared as interfaces without an implementation; a program asking for one is refused with a
diagnostic naming what is available.
