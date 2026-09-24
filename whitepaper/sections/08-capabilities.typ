#import "../template.typ": *

= Capabilities and programs <sec-capabilities>

== Capabilities

Every effect on the world outside the program's own memory requires holding a value that grants it.
Allocation requires a borrow of an allocator; access to a peripheral requires its token, which exists
once; the operating system's services require a borrow of a `System` value; shared mutable state
requires a critical-section token or a synchronized type. There is no global allocator, no ambient
standard output, and no run-time check that a peripheral has been claimed only once: uniqueness is a
fact of the type system, because the runtime constructs the peripherals exactly once and passes them
to the program.

Safe interior mutability exists only through synchronized primitives: atomics, mutexes, and shared
cells accessed with a critical-section token that masks interrupts on single-core targets and
additionally takes a spinlock on multi-core ones. It follows that every value safe to share is safe
to share between threads and interrupt handlers, and no separate thread-safety properties are
required. The cost of this choice is that shared ownership through reference counting must use atomic
counts.

== Programs

A survey of entry mechanisms shows that they reduce to three pieces: a symbol that start-up code
calls, a user definition of that symbol, and an entry symbol at which the linker begins; where they
involve anything more, it is a name the compiler recognizes, or inspection of the root module
@hare-rt @rust-eii. Hosts that state a typed requirement instead, such as Roc platforms and WASI
components, avoid both @roc-platform @wasi-cli. Vun adopts three rules.

#rule[exports][A program is built from a root file, and its output is every declaration marked
`#[link.export]` reachable from that file's bindings. No binding name, signature or build field
denotes an entry point.]

#rule[entry][The linker's entry symbol is data of the target description. A missing or duplicate
definition is reported before linking, with the origin of each definition.]

#rule[typed link symbols][A declaration provided by the linker may be satisfied by an export in the
same program; when both are Vun, their types must be equal.]

A _runtime_ is then an ordinary compile-time function that takes the program's `main` and returns a
struct type of exports. Choosing a runtime is calling it:

```vun
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit { ... };
```

The hosted runtime exports `_start`; a libc runtime exports C `main`; a WASI runtime exports the
command world's `run`; a microcontroller runtime generates, at compile time, the vector table as a
plain array in a link section, the reset handler that initializes memory and constructs the
peripherals, and interrupt wrappers. Trap handling, allocator hooks and log sinks are typed link
symbols satisfied by the runtime, so every trap site in a program reaches its handler by an ordinary,
type-checked link.
