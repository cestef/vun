#import "../template.typ": *

= Conclusion and open problems <sec-conclusion>

Overt's premise is that a systems language can be small in concepts without being small in reach: seven
concepts, each with one rule, cover integer arithmetic without overflow, resource safety without
destructors, borrowing without lifetime annotations, asynchronous code without pinning or allocation,
code generation without a macro language, and program start-up without compiler-known names.

The design questions raised while writing the corpus are settled: returned borrows stay tied to every
borrowed argument unless a parameter is marked `&temp`; integer comparisons narrow the ranges of
immutable bindings; type literals are nominal; `match` is exhaustive; and reference counting is
restricted to atomic counts. What remains to be measured is whether these choices hold up at scale:
whether laziness of code parameters, visible in signatures but not at call sites, causes confusion
in practice, and how scalable vector extensions fit a type indexed by a fixed lane count.
