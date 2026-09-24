#import "../template.typ": *

= Performance model <sec-performance>

Performance is treated as part of the semantics: what the language guarantees determines what an
implementation may assume. The guarantees relevant to code generation are the aliasing facts of
@sec-memory, the static ranges of @sec-integers, the absence of unwinding, the fixed layout of
structs, the specified layout of coroutine frames, and the non-uniqueness of function addresses,
which permits identical instantiations to be merged.

Checks are the main cost of safety in comparable languages. Measurements of Rust attribute roughly
half of the overhead of its run-time checks to bounds checking @rust-checks. In Vun, overflow checks
do not exist, indexing with an index of the right range type is unchecked by construction, and the
checks that remain branch to one cold stub per function carrying a site identifier; the message text
lives in a table that is not loaded on embedded targets, in the manner of deferred logging @defmt.

Performance controls are `lang` attributes and functions, none of which changes meaning: a cold
annotation for functions and branches (branches ending in a call of result type `never` are cold by
rule); inlining requests that are errors when unsatisfiable; per-declaration size optimization;
explicit fused and reassociated floating-point operations with specified reduction order; vector
types whose operations follow the scalar range rules and which lower to scalar code on backends
without vector support; and guaranteed tail calls. A guaranteed tail call must be in return position,
runs pending deferred statements before the jump, and is rejected on a backend that cannot honor it,
never silently degraded.
