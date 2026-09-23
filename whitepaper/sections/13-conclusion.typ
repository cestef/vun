#import "../template.typ": *

= Conclusion and open problems <sec-conclusion>

Overt's premise is that a systems language can be small in concepts without being small in reach: seven
concepts, each with one rule, cover integer arithmetic without overflow, resource safety without
destructors, borrowing without lifetime annotations, asynchronous code without pinning or allocation,
code generation without a macro language, and program start-up without compiler-known names.

Several questions remain open and will be settled with evidence from the example corpus that precedes
the compiler: whether the conservative scoping rule for returned borrows needs a per-parameter
refinement; whether flow-sensitive narrowing of integer ranges is worth making types depend on control
flow; whether laziness of code parameters, visible in signatures but not at call sites, causes
confusion in practice; how scalable vector extensions fit a type indexed by a fixed lane count; and
whether reference counting restricted to atomic counts is an acceptable cost of eliminating
thread-safety traits.
