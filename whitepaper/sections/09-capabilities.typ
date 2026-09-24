#import "../template.typ": *

= Capabilities, safety and programs <sec-capabilities>

== Capabilities

Every effect on the world outside the program's own memory requires holding a value that grants
it (@tab-capabilities). There is no global allocator, no ambient standard output, and no run-time
check that a peripheral has been claimed only once: uniqueness is a fact of the type system,
because the runtime constructs the peripherals exactly once and passes them to the program.

#figure(
  table(
    columns: (auto, 1fr, auto),
    toprule,
    [Capability], [Obtained from], [Grants],
    midrule,
    [`&mut A`, `$A: Allocator`], [an allocator the program constructs, such as a fixed buffer], [allocation],
    [`&Critical`], [`std.sync.critical`, which lends it for one call], [access to `Shared(T)`],
    [peripheral tokens], [the value a board runtime passes to `main`], [memory-mapped I/O],
    [`&mut os.System`], [the value a hosted runtime passes to `main`], [files, sockets, threads, clocks],
    [`&mut Build`], [the build driver], [declaring the build graph],
    bottomrule,
  ),
  caption: [Capabilities and where they come from.],
) <tab-capabilities>

Safe interior mutability exists only through synchronized primitives: atomics, mutexes, and shared
cells accessed with a critical-section token. A critical section is global mutual exclusion,
masking interrupts on single-core targets and additionally taking a spinlock on multi-core ones. It
follows that every value that is safe to share is safe to share between threads and interrupt
handlers, and no separate thread-safety properties are needed. The unsynchronized
`lang.mem.Cell(T)` is usable only in `unsafe` code, and is what makes a type not cell-free. The
cost of this choice is that shared ownership through reference counting must use atomic counts;
single-threaded sharing uses arenas and handles instead.

```vun
ticks: std.sync.Shared(u32) = .init(0);

#[panic.never]
on_systick := () {
    std.sync.critical([] (cs: &temp std.sync.Critical) { ticks.get(cs).* +%= 1; });
};
```

`critical` lends the `Critical` value it creates for the duration of the call only (`&temp`,
@sec-memory). `ticks.get(cs)` returns a `&mut u32` tied to `cs` by the scoping rule, so returning
it from the closure is rejected (@fig-escape): nothing reached through `cs` leaves the critical
section.

#figure(
  ```term
  $ vun check corpus/reject/borrow/critical_escape.vun
  error[borrow.escape]: `cs` is lent for the call only, and a borrow tied to it is returned
    ╭▸ corpus/reject/borrow/critical_escape.vun:9:9
    │
  8 │     sync.critical([] (cs: &temp sync.Critical) -> &mut u32 {
    │                       ── lent as `&temp` here
  9 │         ticks.get(cs) //~ borrow.escape
    │         ━━━━━━━━━━━━━
    │
    ╰ help: a `&temp` parameter never reaches the result; return what another parameter lends
  ```,
  caption: [A borrow obtained inside a critical section cannot leave it.],
) <fig-escape>

The
section itself is implemented by two typed link symbols, `std.sync.enter` and `leave`, which the
runtime exports.

== Traps and function markers

Recoverable errors are `Result(T, E)` values; `e?` propagates them and `e.map_err(f)?` converts
them explicitly. A _trap_ ends the current execution context. The trap sites are a failed `.to`, a
compound assignment out of range, a division by a possibly zero divisor, an unproven index,
unwrapping an absent optional, and `std.panic.fail(reason)`. There is no unwinding. A trap calls
`lang.panic.trap`, a typed link symbol provided by the program's runtime, and passes a 32-bit site
identifier. The table mapping identifiers to origin chains and messages is emitted into a section
that is not loaded, so on embedded targets a trap costs a call and a constant and still reports
the exact source span on the host. `vun check --traps` lists every remaining trap site with the
reason it remains.

A function type carries, as part of the type, a set of _markers_: whether calling it is `unsafe`,
whether it may trap (`panic.never`), a stack bound (`stack.max(n)`), required target features and
its calling convention. Function pointers carry the same markers, so the analyses survive indirect
calls. A table in `lang` gives each marker a relation, which decides where a function is accepted
in place of an expected function type (@tab-markers). Acceptance is computed generically from the
table, so adding a marker adds a row rather than a rule.

#figure(
  table(
    columns: (auto, auto, 1fr),
    toprule,
    [Relation], [Markers], [A function is accepted when it has],
    midrule,
    [duty of the caller], [`unsafe`, `target.require(f)`], [no duty the expected type lacks],
    [guarantee to the caller], [`panic.never`], [every guarantee the expected type has],
    [bound], [`stack.max(n)`], [a bound at most the expected one],
    [identity], [`abi.c`, `abi.interrupt`, `abi.naked`], [the same markers],
    bottomrule,
  ),
  caption: [Function-type markers and their acceptance relations.],
) <tab-markers>

`unsafe` is a single marker with a single meaning: _`unsafe x` is used only where someone vouched
for it_. An `unsafe` expression or block is used where it is written, so writing it vouches for the
unchecked operations inside. A function marked `unsafe` is used at each call, so each call must sit
inside an `unsafe` expression; the function's body receives no permission by being marked. The
unchecked operations are calling such a function, dereferencing a raw pointer, accessing a
top-level `mut` binding, and using the members of a `Cell`.

`panic.never` is checked transitively: the body of a function carrying it may contain no trap site
that range analysis cannot discharge, and may call only functions that carry it. The primitives of
`lang.int` and `lang.mem` that cannot trap carry the marker, as do the corresponding `std`
functions, so interrupt handlers and critical sections can be written under the guarantee. Stack
bounds are computed from the whole-program call graph and the frame sizes the backend reports;
recursion is rejected under a bound unless its depth is bounded at compile time or it is a
guaranteed tail call (@sec-performance).

== Programs

A survey of entry mechanisms shows that they reduce to three pieces: a symbol that start-up code
calls, a user definition of that symbol, and an entry symbol at which the linker begins. Where
they involve anything more, it is a name the compiler recognizes or an inspection of the root
module @hare-rt @rust-eii. Hosts that state a typed requirement instead, such as Roc platforms and
WASI components, avoid both @roc-platform @wasi-cli. Vun adopts three rules.

#rule[exports][A program is built from a root file, and its output is every declaration marked
`#[link.export]` reachable from that file's bindings. No binding name, signature or build field
denotes an entry point.]

#rule[entry][The linker's entry symbol is data of the target description. A missing or duplicate
definition is reported before linking, with the origin of each definition.]

#rule[typed link symbols][A declaration provided by the linker may be satisfied by an export in the
same program; when both are Vun, their types must be equal.]

A _runtime_ is then an ordinary compile-time function that takes the program's `main` and returns
a structure type of exports. Choosing a runtime is calling it:

```vun
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    mut out := sys.stdout();
    std.fmt.print(&mut out, "{} args\n", .{ sys.args().len() });
    .ok
};
```

A hosted runtime exports `_start`; a C-library runtime exports C's `main` and leaves start-up to
the C runtime; a WASI runtime exports the command world's `run`. A microcontroller runtime
generates at compile time the vector table as a plain array in a link section (with a default
handler for every unlisted interrupt, and no weak symbols), the reset handler that initializes
memory and constructs the peripherals once, and the interrupt wrappers:

```vun
start := stm32f4.start(.{
    .main = main,
    .trap = stm32f4.halt,
    .interrupts = .{ .systick = on_systick, .tim2 = stm32f4.time.on_tim2 },
});

main := (p: stm32f4.Peripherals) -> never {
    mut led := p.gpioa.pin(5).output();
    mut timer := stm32f4.time.Timer.init(p.tim2);
    mut a := blink(&mut led, &timer);     // the frame is built in place in main's frame
    std.task.run(.{ &mut a })
};
```

Trap handling, allocator hooks, log sinks, the critical section and the executor's sleep are
typed link symbols that the runtime satisfies, so every trap site reaches its handler through an
ordinary, type-checked link, and every export is checked against every declaration of its symbol
that the file sees. The C-library runtime, for example, implements the critical section by
blocking signals and taking a lock owned by the current thread, and sleeps by yielding the
processor; the STM32F4 runtime saves and restores the interrupt mask and sleeps by waiting for an
interrupt. A library is a root file with exports and no runtime.

== Foreign code

A C import is a declaration provided by the linker,
`#[link.symbol("puts"), abi.c] puts: unsafe (s: *u8) -> i32 = _;`, and `#[link.export("name")]`
exports a binding under a C symbol. Generating declarations from C headers, using the build's real
compiler flags, and emitting C headers for a Vun library are tools rather than language features.
Inline assembly is the `unsafe` function `lang.arch.asm`, which takes a compile-time template and
operands.
