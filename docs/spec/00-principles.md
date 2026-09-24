# 00. Principles

## Thesis

A general-purpose systems language: kernels, firmware, servers, tools, games, anything that wants
control over memory and cost. Nothing in the language assumes an OS, a heap, a word size or an
architecture. The same language scales from a 2 KiB microcontroller to a many-core server and
talks to C, and therefore to every RTOS, IoT SDK and vendor HAL, without friction.

The rule everything else serves:

**The language has a small number of concepts, each with one rule. Every feature is one of those
concepts, a library built on them, or a documented rewrite into them.** If a behavior cannot be
explained by the concept list plus the desugaring table (appendix B), it is a bug in the design.

## The layering rule

```
syntax  --desugars into-->  lang  <--built on--  std  <--built on--  your code
```

- **`lang`** is a package of ordinary Vun source. It declares, with real signatures and doc
  comments, every primitive the compiler provides. A declaration whose value is `_` is provided
  from outside the source: in `lang` by the compiler, elsewhere by the linker or by an `impl`.
  There is no `@builtin` namespace, no magic identifier, no special function name.
- **Syntax desugars only into `lang`.** `a + b`, `&x`, `for`, `yield ..e`, `e?`, `"text"`, `0..n`,
  `#{ }` each map to a named `lang` item. Reading `lang` tells you what the syntax means.
- **`std` is plain library code** on top of `lang`: `Option`, `Result`, allocators, MMIO, `Dyn`,
  executors, formatting, the OS layer, program startup. All of it is replaceable.
- **What is available to `std` is available to you.** Nothing in `std` uses a privilege your code
  lacks.

## Guarantees

1. **Nothing runs that you did not write or call.** No destructors, no implicit fallible
   conversions, no hidden allocation, no unwinding, no global constructors, no auto-deref.
2. **No undefined behavior outside `unsafe`.** Inside `unsafe`, the UB list is finite and
   enumerated (appendix C).
3. **Semantics do not depend on the optimization level.** Debug and release builds compute the
   same results and trap at the same points. The optimizer removes a check only when it proves
   the check cannot fire.
4. **Compile time and run time share one semantics**, including target layout, word size,
   endianness and integer ranges.
5. **Everything is traceable to source.** Every entity in every compiler stage carries an origin
   chain back to the source text that caused it, through desugaring, code generation and lowering
   (spec 10, 13). Diagnostics, debug info and runtime trap reports all read that chain.

## The seven concepts

| # | Concept | One-line rule | Spec |
|---|---------|---------------|------|
| 1 | **Everything is a binding** | Every declaration is `name := value`. Files, directories and packages are structs whose members are their declarations. | 01 |
| 2 | **Types are compile-time values** | A type is a value of type `type`. `$` marks what is known at compile time. Generics, interfaces, reflection, attributes and code generation are ordinary compile-time computation. | 02, 03 |
| 3 | **Integers are ranges** | Every integer type is `Int(lo..hi)`. Arithmetic is exact; only narrowing can fail, and narrowing is explicit. | 04 |
| 4 | **Places have states** | Every storage location is `uninit` or `init`. Moves, initialization, out-parameters, linear resources and `defer` are transitions of one state machine. | 05 |
| 5 | **Borrows are scoped** | A borrow cannot outlive what created it; a returned borrow is tied to all borrowed arguments. There are no lifetime annotations. | 05 |
| 6 | **Capabilities are values** | Allocation, hardware, the OS and interior mutability require holding a value. There is no ambient authority. | 07, 08 |
| 7 | **Functions can suspend** | A function containing `yield` returns a sized frame. Iterators, generators and async tasks are one thing. | 06 |

## Non-goals

- Maximal type-system expressiveness. No higher-kinded types, no type-level programming beyond
  what compile-time evaluation gives for free.
- User-extensible grammar. Extension happens through compile-time code and libraries, so every
  program is read with the same grammar. Code generation is first-class (spec 03), but it
  produces Vun, not new syntax.
- Hiding the machine.
- General algebraic effect handlers. With no heap and statically sized frames, the handler classes
  that can be implemented are already covered: tail-resumptive handlers are capability values,
  abortive handlers are `Result`, one-shot suspension is coroutines (research notes, section 4).

## Taste

These are not rules the compiler checks, but they decide close calls:

- **One way.** When two spellings would mean the same thing, keep one.
- **Organized and deep.** Namespaces nest; names are short because their path gives context
  (`std.mem.alloc.Arena`, not `ArenaAllocator`). Conventions are enforced by lints that ship
  enabled, are configurable per project, and are extensible by libraries (spec 10).
- **Readable diffs.** Generated code is visible through tooling; configuration is code; nothing
  important hides in a file-name convention.
- **Symmetric syntax.** Type syntax mirrors value syntax; patterns mirror constructors; each symbol
  means one thing (spec 01).
