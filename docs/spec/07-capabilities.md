# 07. Capabilities

## 1. Capabilities are values

Anything that touches the outside world requires holding a value that grants it:

| Capability | Where it comes from | Grants |
|------------|---------------------|--------|
| `&mut A` with `$A: Allocator` | you construct one: `mem.alloc.Fixed.init(&mut buf)`, `os.Heap` | allocation |
| `&Critical` | `sync.critical([...] (cs: &Critical) { ... })` | access to `Shared(T)` |
| peripheral tokens | the `Peripherals` value a board runtime passes to `main` | MMIO for that peripheral |
| `&mut os.System` | the value a hosted runtime passes to `main` | files, sockets, threads, clocks, environment |
| `&mut Build` | the build driver | declaring the build graph |

There is no global allocator, no ambient standard output, and no singleton `take()` checked at run
time. Uniqueness of hardware is a type fact: the runtime constructs `Peripherals` exactly once and
passes it to `main` (spec 08).

## 2. Interior mutability is always synchronized

Safe interior mutability exists only through synchronized primitives: `sync.Atomic(T)`,
`sync.Mutex(T)`, and `sync.Shared(T)` accessed with a `&Critical`. A critical section is global
mutual exclusion: interrupts masked on single-core targets, plus a spinlock on multi-core ones.

Consequence: **every value that is safe to share is safe to share across threads and interrupts**,
so no `Send`/`Sync` properties are needed. The unsynchronized `lang.mem.Cell(T)` is usable only in
`unsafe` code, and is what makes a type not cell-free (spec 05).

```ov
ticks: sync.Shared(u32) = .init(0);

#[panic.never]
on_systick := () {
    sync.critical([] (cs: &Critical) { ticks.get(cs).* +%= 1; });
};
```

`ticks.get(cs)` returns `&mut u32` tied to `cs` by the scoping rule, so it cannot escape the
critical section.

## 3. Errors and traps

- **Recoverable errors** are `Result(T, E)` values; `e?` propagates, `e.map_err(f)?` converts.
- **Traps** end the current execution context. Trap sites are: a failed `.to`, a compound
  assignment out of range, a zero divisor, an unproven index, unwrapping `.none`, and
  `std.panic.fail(reason)`, whose compile-time `lang.panic.Reason` (`.message(text)` or
  `.assertion(source, origin)`) makes each distinct reason its own site. There is no unwinding.
- A trap calls `lang.panic.trap`, a typed link symbol that the program's runtime provides
  (spec 08). Every trap site passes a 32-bit **site id**; the message table mapping ids to origin
  chains and text is emitted into a non-loaded section, so traps cost a call and a constant on
  embedded targets and still report the exact source span on the host (spec 10).
- **`#[panic.never]`** on a function is checked transitively: its body may contain no undischarged
  trap site, and it may call only `#[panic.never]` functions.
- `overt check --traps` lists every remaining trap site with the reason it remains.

## 4. Function types

A function type carries, as part of the type, and all checked:

- its ABI: `#[abi.c]`, `#[abi.interrupt]`, `#[abi.naked]`, native by default;
- whether calling it is `unsafe`;
- `#[panic.never]`;
- an optional stack bound `#[stack.max(512)]`;
- required target features `#[target.require(...)]`.

Function pointers carry the same properties, so the analyses survive indirect calls.

`unsafe` is one marker with one meaning: **`unsafe x` is used only where someone vouched for
it.** An `unsafe` expression or block is used where it is written, so writing it vouches for the
unchecked operations inside. A function literal or function type marked `unsafe` is used at each
call, so each call sits inside an `unsafe` expression; the function's body vouches for nothing by
being marked. The marker is the `lang` item `lang.safety.unsafe` (the table `markers`), carried
in the function type; a function without it is accepted where one with it is expected, never the
reverse. Unchecked operations: calling such a function, `.*` on a raw pointer, any access to a
top-level `mut` binding, and a member of a `lang.mem.Cell` (`safety.requires_unsafe`).

### Stack bounds

The compiler computes the worst-case stack use of every function from the whole-program call graph
and the frame sizes the backend reports. Recursion is rejected under a `stack.max` bound unless its
depth is compile-time-bounded (`#[stack.depth(k)]`) or it is a guaranteed tail call (spec 09). A
backend that cannot report frame sizes cannot build a program that uses `stack.max`.

## 5. Foreign code

- A C import is a declaration provided by the linker:
  `#[link.symbol("puts"), abi.c] puts: unsafe (s: *u8) -> i32 = _;`
- `#[link.export("name")]` exports a binding under a C symbol.
- `overt bind foo.h` generates Overt declarations from C headers through libclang, using the
  build's real compile flags. `overt header` emits a C header for an Overt library. Both are tools,
  not language features (spec 12).
- Inline assembly is `lang.arch.asm(template, operands)`, a `lang` function taking compile-time
  data. It is `unsafe`.
