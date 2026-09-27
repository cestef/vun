#import "../template.typ": *

= Introduction <sec-introduction>

Systems languages tend to accumulate special cases. Each is reasonable in isolation. An integer
type wraps because the hardware does; a destructor runs because cleanup is easy to forget; a
lifetime annotation exists because the borrow checker needs a name for a region; a macro language
exists because the base language cannot inspect itself; a pinning wrapper exists because
asynchronous frames must not move; an attribute marks the entry point because the linker needs
one. Together, these mechanisms interact in ways that programmers learn case by case and that
implementations must handle case by case as well.

Vun starts from the opposite constraint. The language is a small set of _concepts_, each governed
by exactly one rule. Every other feature must be one of those concepts, a library built from them,
or a documented rewrite into them. When a behavior cannot be explained by the list of concepts and
the desugaring tables, the design is considered incorrect rather than the explanation incomplete.

The constraint is a means rather than an end. It is intended to make three properties hold at the
same time, properties that existing systems languages tend to trade against each other:

- *Predictable cost.* Every allocation, copy, check and trap site is either written in the source
  or reported by the toolchain. No code runs that the program did not write or call.
- *Safety without annotations.* Memory safety, freedom from data races and freedom from resource
  leaks hold in safe code without lifetime annotations and without a garbage collector.
- *No privileged code.* The standard library, the start-up code of a microcontroller, the test
  runner and dynamic dispatch are ordinary Vun, written with the facilities available to any
  program.

The intended domain is broad: firmware for microcontrollers with a few kilobytes of memory,
operating-system components, language runtimes, servers and command-line tools. Nothing in the
language assumes an operating system, a heap, a word size or an architecture, and interoperation
with C is direct, which gives access to existing real-time operating systems and vendor hardware
abstraction layers.

== Contributions

This paper describes the design of Vun and the architecture of its implementation:

+ A *layering rule* under which every primitive is a declaration in a source package (`lang`),
  syntax desugars only into that package through tables written as Vun data, and the standard
  library holds no privilege (@sec-principles).
+ A surface syntax with fifteen reserved words, in which type syntax mirrors value syntax,
  functions are typed tuple patterns, and a single operator (`$`) expresses every change of
  evaluation stage (@sec-syntax).
+ *Types as compile-time values*, with interfaces defined as functions from a type to a structure
  of requirements, and conformance registered by an attribute on any expression that yields such a
  structure (@sec-types).
+ A *staging model* in which the whole language is available at compile time and in which generic
  code, generated declarations and hygienic quotation share one mechanism, with all generated code
  traceable to its generator (@sec-staging).
+ *Range integers*, whose arithmetic is exact and whose only fallible operation is explicit
  narrowing, so that overflow does not exist as a behavior (@sec-integers).
+ A *memory model* built on one state machine over places, in which moves, initialization,
  out-parameters, linear resources and deferred cleanup are transitions, and in which borrows obey
  a single scoping rule in place of lifetime annotations (@sec-memory).
+ A single *suspension* primitive with statically sized, immovable frames that covers iterators,
  generators and asynchronous tasks without heap allocation (@sec-suspension).
+ A treatment of *capabilities and programs* in which effects require holding values, function
  types carry checked markers, runtimes are ordinary compile-time functions, and typed link symbols
  replace entry-point and handler conventions (@sec-capabilities).
+ An *implementation architecture* in which backends, linkers, targets, passes, evaluators and
  diagnostic renderers are replaceable, compile-time evaluation defines the reference semantics,
  and every entity in every intermediate representation carries an origin chain (@sec-tooling,
  @sec-implementation).

== Status and scope

This is a design paper accompanied by a partial implementation. The specification is a set of
documents with a decision log of 256 entries; a corpus of Vun programs was written before the
compiler and serves as its acceptance test. The implemented part of the compiler covers parsing,
name resolution, type checking, compile-time evaluation, the package system, the checks on the
mid-level representation (place states, kinds, borrows, trap freedom), a low-level
representation, three backends and the linkers they need. Programs run on Linux for x86-64 and
arm64 and under WASI; the standard library, freestanding targets and the editor tools are
incomplete. @sec-implementation reports the state of the implementation in detail.

Statements about run-time and compile-time performance are design targets derived from published
measurements of comparable systems, which are cited where they appear. The few measurements of
Vun itself are reported in @sec-implementation and cover small programs only. Some rules
described here are marked _proposed_ in the decision log, meaning they are implemented and
specified but not yet confirmed; @sec-conclusion lists them.

== Organization

@sec-principles states the layering rule, the guarantees and the seven concepts.
@sec-syntax to @sec-staging describe the surface language, types and staging.
@sec-integers to @sec-capabilities cover the concepts that concern run-time behavior: integers,
memory, suspension and capabilities. @sec-performance states the performance model,
@sec-tooling the diagnostics, testing and package systems, and @sec-implementation the compiler
architecture and its current state. @sec-related discusses related work and @sec-conclusion
concludes with open problems. The appendices give the desugaring table and the list of undefined
behaviors.
