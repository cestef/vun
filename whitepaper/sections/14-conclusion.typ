#import "../template.typ": *

= Conclusion and open problems <sec-conclusion>

Vun's premise is that a systems language can be small in concepts without being small in scope.
Seven concepts, each with one rule, account for integer arithmetic without overflow, resource
safety without destructors, borrowing without lifetime annotations, asynchronous code without
pinning or allocation, code generation without a separate macro language, and program start-up
without names known to the compiler. Because primitives are declarations and desugarings are data,
the compiler contains no list of special names, and the parts of the language that users read and
the parts the compiler acts on are the same files.

== Settled questions

Several questions raised while writing the corpus have been settled, each recorded with its
reasoning in the decision log. Returned borrows remain tied to every borrowed argument, and
parameters that should not be tied are marked `&temp`. Integer comparisons narrow the ranges of
immutable bindings. Type literals are nominal and aliases structural, and anonymous structures
written without an expected type are structural records. `match` is exhaustive. Safe shared
ownership is always synchronized, so reference counts are atomic. Integer intermediates are capped
at 128 bits on every target. Methods of built-in types come from library implementations over type
constructors. `unsafe` and all other properties of function types are markers with declared
acceptance relations.

== Proposed rules

The rules decided most recently are implemented and specified but marked _proposed_ in the
decision log, pending confirmation. Those that concern the language are: a field holding a
function is called as a value, never as a method; compile-time work in a body splits into
expansions run while typing and evaluations run after it; floats, like integers, are one
constructor indexed by a compile-time width; exit statuses are a range every supported system
passes on; a linked symbol names the library it comes from; a fixed-buffer allocator borrows
its storage; and a library's report reached at run time refuses the program that reaches it. The sections above describe these rules as they are implemented.

== Open problems

The following questions remain open or depend on experience that only a larger body of code can
provide.

- *Laziness of code parameters.* A `Code(T)` parameter receives its argument unevaluated. This is
  visible in the signature and in editor hints but not at the call site; whether it causes
  confusion in practice has yet to be measured.
- *Precision of scoped borrows.* Tying a returned borrow to every borrowed argument is simple and
  local, but it may reject programs that a region-based checker accepts. The frequency with which
  `&temp` is needed, and whether it suffices, is an empirical question.
- *Cost of exact arithmetic.* Wide intermediates are reported rather than prevented. Whether this
  is sufficient on 32-bit and smaller cores will be known only once programs run on them; the
  implemented backends target 64-bit machines and 32-bit WebAssembly.
- *Scalable vectors.* Vector extensions without a fixed lane count (SVE, RVV) do not fit a type
  indexed by a compile-time width. A separate type for scalable vectors is planned.
- *Compile-time memory model.* The evaluator models mutable borrows at compile time through loans,
  but not yet borrows held inside aggregate arguments. Its design must preserve determinism and
  memoization.
- *Validation of the performance model.* The claims of @sec-performance rest on measurements of
  other systems. They will be tested against the exit criteria of the roadmap: stack bounds
  confirmed on hardware, and code size within 10% of C on a reference firmware.
