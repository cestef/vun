#import "../template.typ": *

= Principles <sec-principles>

== The layering rule

Vun distinguishes four layers, and the relation between them is the first rule of the design:

#align(center)[
  syntax #h(0.6em) $arrow.r.long$ #h(0.6em) `lang` #h(0.6em) $arrow.l.long$ #h(0.6em) `std`
  #h(0.6em) $arrow.l.long$ #h(0.6em) user code
]

The package `lang` is written in Vun. It declares, with ordinary signatures and documentation,
every primitive the compiler provides: integer and pointer operations, volatile and atomic access,
the coroutine protocol, reflection, and the attributes the compiler acts on. A primitive is a
declaration whose value is the hole `_`: in `lang` the compiler supplies the value; anywhere else
the linker does, which is also how foreign functions are imported. There is no separate namespace
of built-in functions and no identifier whose meaning is fixed by the compiler.

Syntax desugars only into `lang`. Each piece of notation, from `a + b` to `for x in xs` to the
error-propagating postfix `?`, is defined by a rewrite into named `lang` items, listed in a single
table. The standard library `std` is then ordinary code over `lang`, and holds no privilege a user
package lacks: everything it does, from dynamic dispatch to the startup code of a microcontroller,
a user can do the same way.

== Guarantees

The design commits to five properties. Each constrains every later decision.

+ *Nothing runs that the program did not write or call.* There are no destructors, no implicit
  fallible conversions, no hidden allocation, no unwinding, no global constructors and no automatic
  dereferencing.
+ *No undefined behavior outside `unsafe`.* Inside `unsafe`, the conditions that constitute
  undefined behavior form a finite, enumerated list.
+ *Semantics do not depend on optimization.* Debug and release builds of a program compute the same
  results and trap at the same points. An optimizer may remove a check only by proving it cannot
  fire.
+ *Compile time and run time share one semantics*, including the target's layout, word size,
  endianness and integer ranges.
+ *Everything is traceable to source.* Every entity in every compiler stage carries an origin chain
  back to the text that caused it, through desugaring, generation and lowering.

== The concepts

#figure(
  table(
    columns: (auto, 1fr, 2.2fr),
    align: (left, left, left),
    table.hline(),
    [], [Concept], [Rule],
    table.hline(stroke: 0.5pt),
    [1], [Everything is a binding], [Every declaration is `name := value`. Files, directories and packages are structs whose members are their declarations.],
    [2], [Types are compile-time values], [A type is a value of type `type`; `$` marks what is known at compile time.],
    [3], [Integers are ranges], [Every integer type is `Int(lo..hi)`; arithmetic is exact; narrowing is explicit.],
    [4], [Places have states], [Every place is `uninit` or `init`; moves, initialization and consumption are transitions.],
    [5], [Borrows are scoped], [A borrow cannot outlive what created it; a returned borrow is tied to every borrowed argument.],
    [6], [Capabilities are values], [Allocation, hardware, the operating system and interior mutability require holding a value.],
    [7], [Functions can suspend], [A function containing `yield` returns a sized, immovable frame.],
    table.hline(),
  ),
  caption: [The seven concepts of Vun. Every other feature is expressed through them.],
) <tab-concepts>

@tab-concepts is the entire conceptual inventory. Features familiar from other systems languages
appear in it only as consequences. Generics are functions with compile-time parameters (concept 2).
Resource cleanup is the obligation to consume linear values (concept 4). Thread safety follows from
the requirement that interior mutability be obtained through synchronized capabilities (concept 6).
Iterators and asynchronous tasks are both suspension (concept 7).

== Non-goals

Three omissions are deliberate. Vun does not aim for maximal type-system expressiveness: there are
no higher-kinded types, and type-level computation is ordinary compile-time evaluation. It does not
allow user-extensible grammar: code generation produces Vun, never new syntax, so every program is
read with one grammar. And it does not provide general algebraic effect handlers. Without a heap and
with statically sized frames, the classes of handler that can be implemented are tail-resumptive
handlers, which are capability values @effekt; abortive handlers, which are result values; and
one-shot suspension, which is the coroutine primitive. Handlers requiring general resumption need
stack switching or copying @lexa, which would forfeit the guarantee that task memory is known at
build time.
