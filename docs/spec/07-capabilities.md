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

### Storage per thread

A type marked `#[thread.local]` gives every top-level binding of it **one storage per thread**,
each starting as the binding's value; the marker speaks only of top-level bindings, so a value of
the type anywhere else is stored as any other is. `sync.Local(T)` is such a type, which needs no
synchronization since no two threads reach one storage of it:

```vun
depth: sync.Local(u32) = .init(0);

enter := () { depth.set(depth.get() + 1); };
```

A `Local(T)` is reached by value as a `Shared(T)` is (`get`, `set`, `replace`, `update`), never
lending a borrow, and each access is a relaxed atomic one of `lang.mem`: a signal or interrupt
handler the thread runs between two accesses cannot tear a value, and none costs more than a plain
load or store. `T` is a type `Atomic(T)` takes. Where the target runs no threads
(`lang.target.Spec.threads`: bare metal, WASI), the one storage is the program's. A borrow of a
per-thread binding reaches the storage of the thread taking it and is scoped, as a `mut`
binding's is (spec 05, section 4); what the storage holds is known at run time only.

The compiler gives such a binding the target's thread-local storage (ELF's `.tdata`, symbols of
`STT_TLS`), and LIR takes its address with `thread_addr`, never `addr`. C declares it `__thread`;
the native backend reaches it through the TLS descriptor of x86-64 and arm64, whose call gives
the storage's offset from the thread pointer, and which linkers rewrite into that offset in an
executable, so executables and shared libraries share one sequence. 32-bit Arm's native backend
does not reach it yet (it needs `mrc` and literal pools); C does. A runtime starting threads
itself lays out each thread's block and sets the thread pointer (spec 08, section 2).

## 3. Errors and traps

- **Recoverable errors** are `Result(T, E)` values; `e?` propagates, `e.map_err(f)?` converts.
- **Traps** end the current execution context. Trap sites are: a failed `.to`, a compound
  assignment out of range, a zero divisor, an unproven index, unwrapping `.none`, and
  `std.panic.fail(reason)`, whose compile-time `lang.panic.Reason` (`.message(text)` or
  `.assertion(source, origin)`) makes each distinct reason its own site. There is no unwinding.
- A trap calls `lang.panic.trap`, a typed link symbol that the program's runtime provides
  (spec 08). Every trap site passes a 32-bit **site id**, so a trap costs a call and a constant.
- A program whose runtime declares the link symbol `lang.trace.symbols.table` gets the **table
  of its code**, laid out as `lang.trace` declares it: a row per site id (its origin and, where
  the reason is known when compiling, its message); at `lang.trace.Detail.calls`, a row per
  function (where its code starts, its declaration's path and origin, and whether foreign code
  calls it: the C convention, or naked); and a row per origin (file, line, column, the path of
  the declaration it is in, and, for code inlined, the row of the function it is inlined into).
  Lowering makes the table's datum; its rows are written when code is generated, where places
  in source are known.
- The detail is a build option (`std.build.Options.trace`, `--trace`), read at compile time as
  `lang.trace.current`: `off` (no table; a trap writes nothing), `sites`, or `calls`. A debug
  build defaults to `calls`, a release build to `sites`.
- Before ending the program, a runtime's handler writes a **backtrace** to the standard error
  (`std.trace.report`): why it ends (a trap, or a check that failed, naming its
  `lang.check.Violation`), the site's message and place and each function its code is inlined
  into, then, at `calls`, the function of each call on the chain of frame records every function keeps (native code always keeps a frame pointer; C is
  compiled with `-fno-omit-frame-pointer`), past the handler's own frames and up to the first
  function foreign code calls, whose caller's frames need not be chained. On arm64 the saved
  return addresses are stripped of pointer authentication. WebAssembly's frames are not memory a
  program reads: there it names the site alone. A failed check of unsafe code reports the same
  way (spec 13, section 8.7).
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
- A variadic C function is a C import with a pack (spec 01, section 4), declared with the pack of
  the arguments its callers pass: `open: unsafe (path: *u8, flags: u32, ..mode: .{ u32 }) -> i32 =
  _;`. Under `#[abi.c]` the pack's items are C's variadic arguments, `...`, passed as the target's
  convention passes them (spec 13, section 8.5). Vun reads no variadic arguments, so a C function
  with a pack has no body; and each item is what C's default promotions leave, never an `f32` or
  an `f16` (`abi.variadic`).
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
- **Operands are Vun.** A register is named as the table names it, at a width where its name
  gives one (`eax`). An immediate is an integer the operation's width holds, signed or unsigned,
  and the form's immediate holds as it extends it; a constant is never cut to the bits read.
  Memory is `[...]`: one sum on x86-64 (`[base + index * 8 + 16]`), parts on arm64 and Thumb
  (`[x1, 8]`, `[x1, x2]`), where `[x1, 8]!` writes the address back before the access and
  `[x1], 8` after it. A list of registers is the rest of the line (`push r4, lr`, `vpush d8,
  d9`). `'name` is the instruction a label
  `'name:` marks. `{name}` is a declaration's address, relocated when linking. `{in e}` passes
  the value of `e` in a register the backend picks, of the class and width its type needs (a
  float in a floating-point register); `{out p}` writes a register into the place `p`, `{inout
  p}` both, and each takes a place; `{in(rdi) e}` names the register. A bound value's width is its type's
  on the target, so it is checked only where the target's instruction set is the block's
  (`lang.target.Spec.isa`); elsewhere the block is checked for its registers' classes alone.
- **Forms are chosen as the assembler chooses.** A mnemonic spells the forms of its
  instructions, in the table's order, with a condition as its suffix where a form takes one
  (`jz`, `beq`), or after a `.` (`b.eq`), an access width as a suffix (`ldrb`, whose register is
  `w0`: general registers are named at 32 or 64 bits), and qualifiers each after a `.`: a
  Thumb form's encoding width (`add.w` a 32-bit form, `add.n` a 16-bit one, either chosen
  without it) and the types of a floating-point form (`vcvt.s32.f32`); the instruction's width is the one its registers of that width give, and the first form
  taking the operands at that width is the instruction's. Where no operand gives the width and
  the form has several (`mov [rax], 1`), none is chosen.
- **Clobbers are derived.** Each form declares what it writes, so the registers and flags a block
  writes, beyond its outputs, are known without being listed, and the backend saves what it must.
- **Checked when compiling.** A path before `.asm` that is no architecture
  (`asm.unknown_arch`), a mnemonic the table lacks (`asm.unknown_mnemonic`), a word, label or
  expression that is no operand (`asm.unknown_operand`), operands no form of it takes
  (`asm.no_form`), an immediate wider than the form's (`asm.immediate_too_wide`), or a register of
  another class (`asm.register_class`) is an error at the instruction or the operand.
- **Backends.** The native backend encodes the instructions itself; the C backend writes them as
  GNU `__asm__` from the same forms, with the constraints the bindings give, naming a symbol as
  the object does where that is as C does (ELF) and keeping what a block names
  (`__attribute__((used))`); a naked function is `__attribute__((naked))` over plain `__asm__`
  alone, which is all GNU C allows it. A block is compiled
  where the target's code is in its instruction set, `lang.arch.<isa>` for the target's `isa`
  (`thumb` on 32-bit Arm, whose `arch` is `arm`); sema checks a block against the table it
  names whatever the target, so a library may hold blocks for several behind `$if`, and a backend
  given one of another instruction set, or without the block's (wasm), refuses it
  (`backend.lacks`). A form may need what not every processor of its instruction set has
  (`requires: ?Need`, each table's `Need` being `lang.arch.Need(Feature, Size)`: a feature of the
  table's `Feature`, or a floating-point unit computing floats of a width; Thumb's `ldrexd` and
  `strexd` are Arm's application profile's), which the checked block records; a block compiled for a target of its instruction set whose processor lacks one
  (`lang.target.Spec.features`) is refused where it is lowered (`asm.unavailable`), and code picks
  its instructions by `lang.target.has(.feature)` at compile time.

## 7. Checked unsafe code

The place and borrow checks prove safe code. `unsafe` code is vouched for by its writer, and the
reference interpreter checks it while compiling (spec 05, section 6). A program may also be built
with **checks** that catch at run time what the compiler cannot see inside `unsafe`, each of them
a trap carrying its origin:

| Check (`lang.check.Kind`) | Catches |
|---------------------------|---------|
| `pointers` | a read or write through a raw pointer (`lang.mem.read`, `write`, `copy`, `fill`) of bytes no object holds: past the end of an allocation or a stack slot, or freed memory |
| `allocations` | an allocator given back what it was given back already, what it never handed out, or bytes with another size than it handed them out with; with `pointers`, an access to memory given back |
| `asm` | an assembly block writing memory no object holds, or returning with the stack pointer moved |

- **Enabled by the build, never by a flag in the compiler.** `std.build.Options` has
  `checks: &[lang.check.Kind] = &[]`, and `vun build`, `run` and `test` take `--checks`, whose
  values are that table's entries. The checks a program is built with are part of what its
  artifacts are keyed by, so checked and unchecked builds are cached apart.
- **Declared in `lang`, provided by the runtime.** The operations a check performs are typed
  link symbols in `lang.check`, which the program's runtime exports as it exports
  `lang.panic.trap` (spec 08): `access(address, bytes, mode)`, `poison(address, bytes, why)`,
  `allocated(address, bytes)`, `freed(address, bytes)`, `hold(held)`, `evict(allocator)`. A
  runtime without them cannot run a
  checked program: linking fails naming the missing operation and the check needing it
  (`check.unsupported`, declared in `lang/errors.vun` with its page).
- **Checked versions are Vun.** A function, primitive or interface requirement marked
  `#[lang.check.by(kind, f)]` runs as `f` in a program built with `kind` checked: each call of it
  becomes a call of `f`, with the same arguments plus the call's origin as a compile-time
  `lang.meta.Origin`; a function implementing a requirement so marked (the witness's member of
  the same name) runs as `f` too, which then takes the type the impl is for first. `lang.mem.read`
  is marked with `lang.check.mem.read`, which checks the bytes and then reads.
  `std.mem.alloc.Allocator`'s `alloc` and `free` are marked with `std.check.alloc` and `free`,
  so every allocator is checked without cooperating. No library holds a privilege: any of them may mark
  its own operations.
- **Reported as traps with typed reasons.** A failed check calls `lang.check.fail`, a typed link
  symbol taking `&lang.check.Failure { site: u32, violation: Violation, address: usize }` and
  returning `never`. `Violation := enum { outside, freed, returned, double_free, foreign_free,
  size_mismatch, stack_moved }`. The site indexes the message table like a trap site's
  (section 3), so the runtime reports the violation, the address and the origin chain of the
  access. `#[panic.never]` is judged on the program without checks. A check that fails in such
  a function reports behavior that would have been undefined, so it is not a broken promise.
