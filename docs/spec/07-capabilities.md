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
`STT_TLS`; Mach-O's thread-local variables), and LIR takes its address with `thread_addr`, never
`addr`. How a platform reaches it is part of its calling convention (`vun_abi::Thread`: the
sequence, the register it gives the address in, those it overwrites). C declares it `__thread`;
on ELF the native backend reaches it through the TLS descriptor of x86-64 and arm64, whose call gives
the storage's offset from the thread pointer, and which linkers rewrite into that offset in an
executable, so executables and shared libraries share one sequence. 32-bit Arm's native backend,
whose code reaches what the linker places by its address and so builds executables alone, adds to
the thread pointer `mrc` reads (`TPIDRURO`) the storage's offset from it, a word the linker writes
beside the code, which loads it and branches around it. A relocation naming the storage names a
symbol of it, never its section's, which linkers read as an address. On Mach-O each binding is a
thread-local variable: its symbol names a descriptor in `__thread_vars` (the loader's
`_tlv_bootstrap`, a key, the address of its initial bytes in `__thread_data`), and code calls the
function the descriptor holds, which gives the calling thread's storage. A runtime starting threads
itself lays out each thread's block and sets the thread pointer (spec 08, section 2).

## 3. Errors and traps

- **Recoverable errors** are `Result(T, E)` values; `e?` propagates, `e.map_err(f)?` converts.
- **Traps** end the current execution context. Trap sites are: a failed `.to`, a compound
  assignment out of range, a zero divisor, an unproven index, unwrapping `.none`, and
  `std.panic.fail(reason)`, whose compile-time `lang.panic.Reason` (`.message(text)` or
  `.assertion(source, origin)`) makes each distinct reason its own site, placed where an
  assertion's condition is written. There is no unwinding.
- **The execution context is the thread.** A trap in a thread a runtime spawned ends that thread
  alone: the handler keeps the trap where the runtime keeps how the thread ended
  (`std.os.thread.Ended`), then ends the thread, and `Thread.join` gives `.err(trap)` in place of
  what the closure returned. A trap in the thread the program started in, or on a target without
  threads, ends the program. Nothing runs on the way out: `defer` does not run, and what the thread
  held stays held. At compile time a trap ends the evaluation of the call it is in: the same rule.
  A thread reports its traps where they fire, a backtrace on the standard error, unless it sets
  `std.os.thread.reporting` to `false`, leaving them to what joins it: a property test's search
  runs its cases so, and reports only the smallest failing one (spec 11, section 2).
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
  return addresses are stripped of pointer authentication. In a thread, the chain ends where the
  thread starts (`Thread.run`); the backtrace goes on, after `spawned by`, with the calls that
  spawned it, which `Thread.spawn` takes (`std.trace.Spawn`, empty below `calls`) and the thread
  keeps while it runs, pointed at by `std.trace.spawner`, a `Local`. WebAssembly's frames are not
  memory a program reads: there it names the site alone. A failed check of unsafe code reports the same
  way (spec 13, section 8.7).
- **`#[panic.never]`** on a function is checked transitively: its body may contain no undischarged
  trap site, and it may call only `#[panic.never]` functions. A closure called in a
  `#[panic.never]` function without the marker keeps the promise when its own body does: no
  undischarged trap site, and calls only of functions that keep it; declared functions keep it
  only by the marker.
- `vun check --traps` lists every remaining trap site with the reason it remains.

## 4. Function types

A function type carries, as part of the type, and all checked:

- its ABI: `#[abi.c]`, `#[abi.interrupt]`, `#[abi.naked]`, native by default; or a calling
  convention of the target's architecture that the target's own C convention is not
  (`#[abi.sysv]` and `#[abi.win64]` of x86-64, `#[abi.cdecl]`, `#[abi.stdcall]`,
  `#[abi.fastcall]` and `#[abi.thiscall]` of 32-bit x86), a function so marked being called as a
  C function is under that convention. A convention of another architecture is refused where it
  is lowered, and a backend that does not have them (`Feature::Conventions`, the C backend's)
  refuses a program naming one that is not the target's;
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
| identity | `abi.c`, `abi.interrupt`, `abi.naked`, `abi.sysv`, `abi.win64`, `abi.cdecl`, `abi.stdcall`, `abi.fastcall`, `abi.thiscall` | the same ones |

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
- `vun ffi bind foo.h --cflags "..."` generates Vun declarations from C headers through
  libclang, loaded when the command runs (decision 131), with the build's real compile flags
  (decision 134). Each binding is named in the case of what it binds (spec 10, section 5): C's
  `color_t` is `ColorT`, `MODE_READ` is `mode_read`, and a function or variable keeps its C name
  for the linker through `link.symbol`. `vun ffi header` emits a C header for a Vun library. Both are tools, not
  language features (spec 12).
- Inline assembly is an `unsafe` call of `lang.arch.asm` on a block of an architecture's
  instructions, `lang.arch.asm({ ... })` (section 6).

## 6. Inline assembly

An architecture's instruction set is data in `lang.arch.<arch>` (`lib/lang/lang/arch/`): its
registers by class, and each instruction's forms, as the native backend's assemblers use them
(spec 13, section 8.3), with the registers and flags each form reads and writes. The toolchain's
assembler tables are generated from these declarations, so the language, the assembler and the
listings share one table. A block of instructions is ordinary Vun: a block of calls, each calling
an instruction the table declares, passed as code.

```vun
entry := () -> never {
    unsafe lang.arch.asm({
        ..lang.arch.x86_64.names;
        mov(rdi, rsp);
        \and(rsp, -16);
        call(_boot);
    })
};

add3 := (a: u64) -> u64 {
    mut r: u64 = 0;
    unsafe lang.arch.asm({ ..lang.arch.x86_64.names; lea(out(r), mem(in(a), 3)); });
    r
};

countdown := (n: u32) -> u32 {
    mut left: u32 = n;
    unsafe lang.arch.asm({
        ..lang.arch.thumb.names;
        top := here();
        subs(inout(left), inout(left), 1);
        b(ne, top);
    });
    left
};
```

- **The names are the architecture's declarations.** `lang.arch.<arch>` declares each instruction
  as a value of its `Instruction`, or of its `Member` for one of a `Family` sharing its forms, and
  a `Mnemonic` for a name spelling several (`b` is `b_always`, or `bcond` given a condition). An
  enum naming registers says what it names as its attribute (`#[Registers{ .class = .general,
  .size = .some(.b32) }] R32 := enum { eax, ... }`), an enum of named choices its kind
  (`#[Choice.barrier]`). The namespace `lang.arch.<arch>.names` spreads what a block is written
  with: the operand forms, the enums of registers at each width (`eax`, `w0`, `d8`), the
  conditions (`ne`), the choices (`primask`, `sy`, `ish`) and the instructions, whose variants
  and members a spread brings (spec 01, section 8). A block of instructions starts with the spread
  (`lang.arch.asm({ ..lang.arch.x86_64.names; ... })`), which brings them into that block alone,
  looked up after the names of the file and of the scopes around it, so a parameter `a` stays the
  parameter and the condition is `Cond.a`, and a function of the file named as an instruction is
  reached instead. The declarations' names are the assembler's, escaped where they are reserved
  (`\and`).
- **A block is `lang.arch.asm({ ... })`.** `lang.arch.asm` takes the block as code (`$code:
  Code(())`, spec 03, section 4) and is `unsafe`; each statement of the block calls an instruction,
  declares a label, or spreads the names. An instruction is callable with any operands, its
  `lang.ops.Call` impl covering every argument tuple (`_`, spec 02, section 3), so the block is
  typed as any code is; sema then reads the typed block against the table of the instructions'
  architecture, into typed values of it. The call is of type `()`, or `never` when its last
  instruction does not return. Outside a block an instruction cannot run: its call reports through
  `lang.diag.error`, so a program reaching one is refused at the call that led to it
  (`comptime.reported`, spec 10, section 3).
- **Operands are values.** A register is a variant of its width's enum. An immediate is an integer
  known at compile time, which the operation's width holds, signed or unsigned, and the form's
  immediate holds as it extends it; a constant is never cut to the bits read. A declaration's
  address (a function, or `&x` of an item) is relocated when linking. `in(e)` passes the value of
  `e` in a register the backend picks, of the class and width its type needs (a float in a
  floating-point register); `out(p)` writes a register into the place `p`, `inout(p)` both;
  `.at(rdi)` names the register instead (`in(e).at(rdi)`, `Operand.at`). Memory is `mem(...)` of its
  parts: a base, then an index (`scaled(rcx, 8)` on x86-64) and an offset (`mem(rax, scaled(rcx,
  8), 16)`, `mem(x1, 8)`, `mem(x1, x2)`); `pre(mem(x1, 8))` writes the address back before the
  access (`[x1, 8]!`), `post(mem(x1), 16)` after it (`[x1], 16`). A list of registers is the
  operands themselves (`push(r4, r5, lr)`). A label is a value: `top := here();` marks the next
  instruction, `end := label();` makes one a later `place(end);` marks, and a jump takes it
  (`b(ne, top)`). A bound value's width is its type's on the target, so it is checked only where
  the target's instruction set is the block's (`lang.target.Spec.isa`); elsewhere the block is
  checked for its registers' classes alone.
- **Forms are chosen as the assembler chooses.** A call writes what its assembler spells into the
  mnemonic before its operands: types, then a Thumb form's encoding width (`add(w, r0, r1, r2)`, a
  32-bit form; `n` a 16-bit one; either chosen without it). A condition is an operand, the first
  (`b(ne, top)`, `j(z, top)`, x86-64's `jz`; `b(w, eq, end)`). A width a form is defined at lists
  the types that select it where its spelling names it apart (`Sized.types`: `&[u8]` for `ldrb`,
  `&[i32, f32]` for `vcvt.s32.f32`), and is selected by a call writing them (`ldr(u8, w1, mem(p,
  3))`, `vcvt(i32, f32, s0, s1)`); one listing none, by a call writing none or the one type of its
  width (`adds(f64, ...)` on x86-64). The assemblers' spelling of the types is derived from them
  where the tables are generated. The instruction's width is the one its registers of that width
  give, and the first form, in the table's order, taking the operands at that width is
  the instruction's. Where no operand gives the width and the form has several (`mov(mem(rax),
  1)`), none is chosen.
- **Clobbers are derived.** Each form declares what it writes, so the registers and flags a block
  writes, beyond its outputs, are known without being listed, and the backend saves what it must.
- **Checked when compiling.** Instructions of more than one architecture in a block
  (`asm.unknown_arch`), a call of what is no instruction (`asm.unknown_mnemonic`), an operand that
  is none of the above (`asm.unknown_operand`), operands no form of it takes (`asm.no_form`), an
  immediate wider than the form's (`asm.immediate_too_wide`), or a register of another class
  (`asm.register_class`) is an error at the instruction or the operand.
- **Backends.** The native backend encodes the instructions itself; the C backend writes them as
  GNU `__asm__` from the same forms, with the constraints the bindings give, naming a symbol as
  the object does where that is as C does (ELF) and keeping what a block names
  (`__attribute__((used))`); a naked function is `__attribute__((naked))` over plain `__asm__`
  alone, which is all GNU C allows it. A block is compiled
  where the target's code is in its instruction set, `lang.arch.<isa>` for the target's `isa`
  (`thumb` on 32-bit Arm, whose `arch` is `arm`); sema checks a block against its architecture's
  table whatever the target, so a library may hold blocks for several behind `$if`, and a backend
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
