# 07. Capabilities

## 1. Capabilities are values

Anything that touches the outside world requires holding a value that grants it:

| Capability | Where it comes from | Grants |
|------------|---------------------|--------|
| `&mut A` with `$A: Allocator` | you construct one: `mem.alloc.Fixed.init(&mut buf)`, `os.Heap` | allocation |
| `&Critical` | `sync.critical([...] (cs: &temp Critical) { ... })` | access to `Shared(T)` |
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

```vun
ticks: sync.Shared(u32) = .init(0);

#[panic.never]
on_systick := () {
    sync.critical([] (cs: &temp sync.Critical) { ticks.set(cs, (ticks.get(cs) + 1).wrap(u32)); });
};
```

`sync.critical` takes `$F: lang.ops.Call((&temp Critical), R)` and lends the `Critical` it makes
for the call only (spec 05, section 4), so returning it from the closure is `borrow.escape`. A
`Shared(T)` is reached only by value: `get` copies the value out, `set` and `replace` put one in,
and `update` puts in what a function makes of a copy. It never lends a borrow of what it holds,
since sections nest (`enter` records whether it took the section) and a borrow lent to each of two
nested sections would alias. The section itself is two typed link symbols the runtime exports,
`std.sync.enter` and `leave` (spec 08, section 1). `Shared(T)` and `Atomic(T)` hold a
`lang.mem.Cell(T)`, reached through `lang.mem.cell_pointer` and the `lang.mem.atomic_*`
primitives in `unsafe` code; `Mutex(T)` spins on an `Atomic(bool)` and returns a linear `Guard`.

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
  trap site, and it may call only `#[panic.never]` functions. A closure called in a
  `#[panic.never]` function without the marker keeps the promise when its own body does: no
  undischarged trap site, and calls only of functions that keep it; declared functions keep it
  only by the marker.
- `vun check --traps` lists every remaining trap site with the reason it remains.

## 4. Function types

A function type carries, as part of the type, and all checked:

- its ABI: `#[abi.c]`, `#[abi.interrupt]`, `#[abi.naked]`, native by default;
- whether calling it is `unsafe`;
- `#[panic.never]`;
- an optional stack bound `#[stack.max(512)]`;
- required target features `#[target.require(...)]`.

Function pointers carry the same properties, so the analyses survive indirect calls.

Each property is a **marker**: a `lang` attribute, with its arguments, that the function type
carries (`#[panic.never] (u8) -> u8`, a function literal or a declaration marked so). The table
`fn_markers` of `desugar.vun` names them and gives each a relation, which decides where a function
is accepted in place of an expected function type:

| Relation | Markers | Accepted when the function has |
|----------|---------|--------------------------------|
| duty (of the callers) | `unsafe`, `target.require(f)` | no duty the expected type lacks |
| guarantee (to the callers) | `panic.never` | every guarantee the expected type has |
| bound | `stack.max(n)` | a bound at most the expected one |
| identity | `abi.c`, `abi.interrupt`, `abi.naked` | the same ones |

`unsafe` is one marker with one meaning: **`unsafe x` is used only where someone vouched for
it.** An `unsafe` expression or block is used where it is written, so writing it vouches for the
unchecked operations inside. A function literal or function type marked `unsafe` is used at each
call, so each call sits inside an `unsafe` expression; the function's body vouches for nothing by
being marked. The marker is the `lang` item `lang.safety.unsafe` (the table `markers`), carried
in the function type; a function without it is accepted where one with it is expected, never the
reverse. Unchecked operations: calling such a function, `.*` on a raw pointer, and any access to
a top-level `mut` binding (`safety.requires_unsafe`). A `lang.mem.Cell` is reached only through
`unsafe` functions: `lang.mem.cell_pointer`, and `get` and `set` of `std.mem`.

### Stack bounds

The compiler computes the worst-case stack use of every function from the whole-program call graph
and the frame sizes the backend reports: a function's frame and the most any call it makes takes.
Recursion is rejected under a `stack.max` bound unless its depth is compile-time-bounded
(`#[stack.depth(k)]`) or it is a guaranteed tail call (spec 09); so is a call through an address. A
declaration provided from outside (an import, `= _` with `link.symbol`) states the stack a call of
it takes with `#[stack.max(n)]`, trusted as its signature is; one that states none is unbounded.
The declarations of one symbol that the program sees are one import, bound by the largest they
state, so a program declares the stack of what it imports itself, the trap handler
`lang.panic.trap` included. A
backend that cannot report frame sizes cannot build a program that uses `stack.max`
(`stack.exceeded`, `stack.unbounded`, `stack.unreported`, checked when the program is built).

## 5. Foreign code

- A C import is a declaration provided by the linker:
  `#[link.symbol("puts"), abi.c] puts: unsafe (s: *u8) -> i32 = _;`
- `#[link.export("name")]` exports a binding under a C symbol.
- `vun tool bind foo.h --cflags "..."` generates Vun declarations from C headers through
  libclang, loaded when the command runs (decision 131), with the build's real compile flags
  (decision 134). `vun tool header` emits a C header for a Vun library. Both are tools, not
  language features (spec 12).
- Inline assembly is an `unsafe` block of an architecture's instructions, `lang.arch.<arch>.asm
  { ... }` (section 6).

## 6. Inline assembly

An architecture's instruction set is data in `lang.arch.<arch>` (`lib/lang/lang/arch/`): its
registers by class, and each instruction's forms, as the native backend's assemblers use them
(spec 13, section 8.3), with the registers and flags each form reads and writes. The toolchain's
assembler tables are generated from these declarations, so the language, the assembler and the
listings share one table.

```vun
entry := () -> never {
    unsafe lang.arch.x86_64.asm {
        mov rdi, rsp
        and rsp, -16
        call {_boot}
    }
};

add3 := (a: u64) -> u64 {
    mut r: u64 = 0;
    unsafe lang.arch.x86_64.asm {
        lea {out r}, [{in a} + 3]
    };
    r
};
```

- **A block of instructions.** `lang.arch.<arch>.asm { ... }` holds one instruction per line in the
  architecture's usual syntax: the mnemonic, then operands separated by `,` up to the line's end,
  `[...]` holding a memory operand's parts. `asm` is a contextual word, the block's only when `{`
  follows it where a brace continues an expression, so `if c.asm { }` reads the member `asm`.
  Immediates are plain integers on every architecture (`add x0, x0, 1`, no `#`). It is read at
  compile time against the architecture's table, into typed values of it; the block is an expression
  of type `()`, or `never` when its last instruction does not return. It is `unsafe`.
- **Operands are Vun.** A register is named as the table names it. An immediate is an integer
  expression the form's width must hold. `{name}` is a declaration's address, relocated when
  linking. `{in e}` passes the value of `e` in a register the backend picks, `{out p}` writes a
  register into the place `p`, `{inout p}` both; `{in(rdi) e}` names the register.
- **Clobbers are derived.** Each form declares what it writes, so the registers and flags a block
  writes, beyond its outputs, are known without being listed, and the backend saves what it must.
- **Checked when compiling.** A mnemonic the table lacks, operands no form of it takes, an
  immediate wider than the form's, or a register of another class is an error at the instruction.
- **Backends.** The native backend encodes the instructions itself; the C backend writes them as
  GNU `__asm__` from the same forms, with the constraints the bindings give; a backend for an
  architecture without the block's (wasm) refuses it (`backend.lacks`).

