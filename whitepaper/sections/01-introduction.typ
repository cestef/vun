#import "../template.typ": *

= Introduction

Systems languages accumulate special cases. Each is locally reasonable: an integer type that wraps
because the hardware does, a destructor that runs because forgetting cleanup is common, a lifetime
annotation because a borrow checker needs a name for a region, a macro language because the base
language cannot inspect itself, a pinning wrapper because asynchronous frames must not move, an
attribute that marks the entry point because the linker needs one. Taken together, they turn a
language into a set of mechanisms that interact in ways a programmer must learn case by case, and
that an implementation must special-case in turn.

Vun starts from the opposite constraint. The language is a small set of _concepts_, each governed
by exactly one rule, and every other feature must be either one of those concepts, a library built
from them, or a documented rewrite into them. When a behavior cannot be explained by the concept
list and the desugaring table, the design is considered wrong, not the explanation incomplete.

The constraint is not minimalism for its own sake. It is what makes three properties achievable at
once, properties that systems programmers usually trade against each other:

- *Predictable cost.* Every allocation, copy, check and trap site is either visible in the source or
  reported by the toolchain; nothing runs that the program did not write or call.
- *Safety without annotation burden.* Memory safety, freedom from data races and freedom from
  resource leaks hold in safe code, with no lifetime annotations and no garbage collector.
- *No privileged code.* The standard library, the startup code for a microcontroller, the test
  runner and the dynamic dispatch machinery are ordinary Vun, written with the same facilities
  available to any program.

== Contributions

This paper describes the design of Vun and the architecture of its implementation. Specifically:

+ A *layering rule* under which every primitive is a declaration in a source package (`lang`),
  syntax desugars only into that package, and the standard library holds no privilege
  (@sec-principles).
+ A surface syntax with fifteen reserved words, in which type syntax mirrors value syntax, functions
  are typed tuple patterns, and one operator (`$`) expresses every change of evaluation stage
  (@sec-syntax).
+ A *staging model* in which the whole language is available at compile time, and generic code,
  generated declarations and hygienic quotation share one mechanism, with all generated code
  traceable to its generator (@sec-staging).
+ *Range integers*, whose arithmetic is exact and whose only fallible operation is explicit
  narrowing, eliminating overflow as a class of behavior (@sec-integers).
+ A *memory model* built on one state machine over places, in which moves, initialization,
  out-parameters, linear resources and deferred cleanup are transitions, and borrows are scoped by
  a single rule that replaces lifetime annotations (@sec-memory).
+ A single *suspension* primitive with statically sized, immovable frames that covers iterators,
  generators and asynchronous tasks without heap allocation (@sec-suspension).
+ A treatment of *programs as their exports*, with runtimes as ordinary compile-time functions and
  typed link symbols replacing entry-point and handler magic (@sec-capabilities).
+ An *implementation architecture* in which backends, linkers, targets, passes, evaluators and
  diagnostic renderers are replaceable, compile-time evaluation defines the reference semantics,
  and every entity in every intermediate representation carries an origin chain
  (@sec-tooling, @sec-implementation).

== Status

This is a design paper. The specification exists as a set of documents; the compiler is at the
start of its first phase. Statements about performance are design targets derived from measurements
of comparable systems, cited where they appear, and not measurements of Vun.
