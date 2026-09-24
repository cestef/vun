#import "../template.typ": *

= Implementation architecture <sec-implementation>

The compiler is a set of Rust libraries. Source is parsed into token and postorder syntax-tree arrays;
desugared into a high-level representation with names resolved; analyzed on demand, with type checking
and compile-time evaluation interleaved because types are values; lowered to a mid-level
representation (MIR), a control-flow graph over places on which all checks of @sec-memory run; and
lowered again to a low-level representation (LIR), a type-erased static-single-assignment form shared
by every backend.

Three decisions shape the architecture. First, *the reference semantics is executable*: an interpreter
of MIR under an explicit target model runs compile-time code and tests, and every backend is checked
against it by differential testing on a corpus and on randomly generated programs @csmith. Second,
*every stage after the front end is replaceable*: backends, linkers, external tools, passes, evaluators
and diagnostic renderers are interfaces; targets are data written in Vun; and external backends and
linkers are processes that exchange LIR text and a linker-independent plan. The initial backends are C,
for reach; Cranelift, for fast debug builds; QBE, for simplicity; and LLVM, for optimized code. Third,
*speed is structural*: data-oriented layouts, which reduced Zig's front-end time by 20 to 40 percent
@zig-devlog; incremental queries at the granularity of declarations with early cutoff on interface
hashes; parallel analysis and code generation from the first phase; and merging of instantiations whose
type-erased LIR is identical. Measurements elsewhere show that once a backend is fast, the front end
dominates @tpde, so these choices are made before the first line of the front end is written.

The grammar is defined once, as Vun data, and generates the parser's syntax kinds, the syntax types
available to compile-time code, the formatter's tables and the editor grammar, so that tokens and nodes
cannot drift between the compiler and the language's own view of itself.
