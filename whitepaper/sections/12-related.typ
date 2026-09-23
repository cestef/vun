#import "../template.typ": *

= Related work <sec-related>

*Compile-time evaluation.* Zig demonstrated that a single language can serve for generic programming
and code generation through compile-time evaluation @zig-0-16, and its experience informs Overt's
evaluator design @zig-4055. Jai extends compile-time execution to arbitrary input and output, at the
cost of cacheability. C++26 adds static reflection with declarations as data @p2996 and has deferred
token-based injection @p3294. Typed multi-stage programming in MetaOCaml @metaocaml and Scala 3
@scala-macros is the model for Overt's quotations; hygiene by scope sets @sets-of-scopes
@lean-notations is the alternative Overt avoids by not splicing raw syntax.

*Memory.* Rust established that ownership and borrowing can provide memory safety without a collector;
Overt keeps exclusivity and drops lifetime annotations in favor of scoped borrows, in the spirit of the
parameter conventions of Hylo @hylo. Austral makes linear types the primary resource discipline
@austral, as Overt does for every linear value.

*Effects.* Capability-passing compilation of effect handlers @effekt and lexically scoped handlers
@lexa show how far effects can be made zero-cost; Overt adopts their capability view and declines
general handlers.

*Tooling.* Go's minimal version selection @go-mvs, checksum files, analysis-driven fixes @go-fix, Zig's
content-addressed packages @unboxed-zig, defmt's deferred formatting @defmt, and rustc's and Ruff's fix
applicability @rustc-diag @ruff-fixes are adopted with small changes.
