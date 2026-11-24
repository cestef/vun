#import "../template.typ": *
#import "../figures.typ": layering

= Principles <sec-principles>

== The layering rule

Vun distinguishes four layers, and the relation between them is the first rule of the design
(@fig-layering).

#figure(
  layering,
  caption: [The layering rule. `lang` is the only package the compiler knows; `std` and user code
    have the same access to it.],
) <fig-layering>

Syntax desugars into `lang`. `std` and user code are built on `lang`. User code is built on `std` by
choice.

The package `lang` is written in Vun. It declares, with ordinary signatures and documentation
comments, every primitive the compiler provides: integer and pointer operations, volatile and
atomic access, the coroutine protocol, reflection, and the attributes the compiler acts on. A
primitive is a declaration whose value is the hole `_`. Inside `lang` the compiler supplies the
value; elsewhere the linker supplies it, which is also how foreign functions are imported; inside
an interface, a conforming implementation supplies it. There is no separate namespace of built-in
functions and no identifier whose meaning is fixed by the compiler. The integer narrowing
primitives, for instance, are declared as follows.

```vun
// lang/int.vun
unsigned := ($bits: Int(1..)) -> type { Int(0..1 << bits) };
u32 := unsigned(32);

/// `x` as a value of `T`, trapping when it is outside `T`.
to: ($r: Range, x: Int(r), $T: type) -> T = _;
/// `x` reduced modulo the size of `T` into its range.
#[panic.never]
wrap: ($r: Range, x: Int(r), $T: type) -> T = _;
```

The compiler learns which primitives exist by reading `lang`: a generator produces, for every
namespace of `lang`, an enumeration of its `= _` declarations, so a primitive the compiler does not
implement is a build error of the compiler itself.

Syntax desugars only into `lang`. Each piece of notation, from `a + b` to `for x in xs` to the
postfix `?`, is defined by a rewrite into named `lang` items. The rewrites are tables in the `lang` source file `syntax/desugar.vun`, which the compiler reads
as data. A table names `lang` items by path, so renaming an item that a form desugars into is a compile error. An excerpt of the table of operators and the table of borrow forms:

```vun
operators := .{
    .{ .token = .plus, .function = lang.ops.Add.add },
    .{ .token = .lt, .function = lang.ops.PartialOrd.lt },
    // ...
};
borrows := .{
    .{ .mode = [], .item = lang.mem.Ref, .before = .init, .after = .init,
       .access = .shared, .extent = .tied },
    .{ .mode = [.out], .item = lang.mem.RefOut, .before = .uninit, .after = .init,
       .access = .exclusive, .extent = .tied },
    // ...
};
```

Reading `lang` therefore tells a programmer what the syntax means, and changing the meaning of a
form is a change to a data file that the compiler, the documentation and the checks all consume.
The complete desugaring is given in @app-desugar.

The standard library `std` is ordinary code over `lang` and holds no privilege that a user package
lacks. Everything it does, from dynamic dispatch and executors to the start-up code of a
microcontroller, a user package can do in the same way. The dependencies `lang`, `std` and
`targets` are declared by default and can be replaced.

== Guarantees

The design commits to five properties. Each constrains the decisions in later sections.

+ *Nothing runs that the program did not write or call.* There are no destructors, no implicit
  fallible conversions, no hidden allocation, no unwinding, no global constructors and no automatic
  dereferencing beyond the single rule of @sec-syntax-places.
+ *No undefined behavior outside `unsafe`.* Inside `unsafe`, the conditions that constitute
  undefined behavior form a finite, enumerated list (@app-ub).
+ *Semantics do not depend on optimization.* Debug and release builds of a program compute the
  same results and trap at the same points. An optimizer may remove a check only by proving that it
  cannot fire.
+ *Compile time and run time share one semantics*, including the target's layout, word size,
  endianness and integer ranges.
+ *Everything is traceable to source.* Every entity in every compiler stage carries an origin chain
  back to the text that caused it, through desugaring, generation and lowering. Diagnostics, debug
  information and run-time trap reports all read that chain.

== The concepts

#figure(
  table(
    columns: (auto, 0.9fr, 2.2fr, auto),
    align: (right, left, left, left),
    toprule,
    [], [Concept], [Rule], [Section],
    midrule,
    [1], [Everything is a binding], [Every declaration is `name := value`. Files, directories and packages are structures whose members are their declarations.], [@sec-syntax],
    [2], [Types are compile-time values], [A type is a value of type `type`; `$` marks what is known at compile time. Generics, interfaces, attributes, reflection and code generation are compile-time computation.], [@sec-types, @sec-staging],
    [3], [Integers are ranges], [Every integer type is `Int(lo..hi)`; arithmetic is exact; narrowing is explicit.], [@sec-integers],
    [4], [Places have states], [Every place is `uninit` or `init`; moves, initialization, out-parameters, consumption and `defer` are transitions of one state machine.], [@sec-memory],
    [5], [Borrows are scoped], [A borrow cannot outlive what created it; a returned borrow is tied to every borrowed argument. There are no lifetime annotations.], [@sec-memory],
    [6], [Capabilities are values], [Allocation, hardware, the operating system and interior mutability require holding a value. There is no ambient authority.], [@sec-capabilities],
    [7], [Functions can suspend], [A function containing `yield` returns a sized, immovable frame.], [@sec-suspension],
    bottomrule,
  ),
  caption: [The seven concepts of Vun. Every other feature is expressed through them.],
) <tab-concepts>

@tab-concepts is the complete conceptual inventory. Features familiar from other systems
languages are consequences of it. Generics are functions with compile-time parameters
(concept 2). Resource cleanup is the obligation to consume linear values (concept 4). Thread
safety follows from the requirement that interior mutability be obtained through synchronized
capabilities (concept 6). Iterators and asynchronous tasks are both suspension (concept 7).

== Non-goals

Four omissions are deliberate.

- *Maximal type-system expressiveness.* There are no higher-kinded types, and type-level
  computation is ordinary compile-time evaluation.
- *User-extensible grammar.* Code generation produces Vun, never new syntax, so every program is
  read with one grammar.
- *Hiding the machine.* Layout, calling conventions, memory placement and trap sites are specified
  and visible.
- *General algebraic effect handlers.* Without a heap and with statically sized frames, the classes
  of handler that can be implemented are tail-resumptive handlers, which are capability values
  @effekt; abortive handlers, which are result values; and one-shot suspension, which is the
  coroutine primitive. Handlers requiring general resumption need stack switching or copying
  @lexa, which would give up the guarantee that task memory is known at build time.

== Criteria for close decisions

Where the rules leave a choice open, four criteria decide it. There should be _one way_ to write
each thing: when two spellings would mean the same, one is kept. Namespaces are _deep and
organized_: names are short because their path gives context (`std.mem.alloc.Arena`, not
`ArenaAllocator`), and conventions are enforced by lints that are enabled by default,
configurable and extensible. Changes should produce _readable diffs_: generated code is visible
through tooling, configuration is code, and no behavior depends on a file-name convention. Syntax
is _symmetric_: types mirror values, patterns mirror constructors, and each symbol has one meaning.
