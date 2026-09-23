# Overt: language design, draft 1

> Working name. Rename freely; nothing below depends on it. Source extension `.ov`, CLI `overt`.

## 0. Thesis

A general-purpose systems language: kernels, firmware, servers, tools, games, anything that wants
control over memory and cost. Nothing in the language assumes an OS, a heap, a word size or an
architecture, so the same language scales from a 2 KiB microcontroller to a 128-core server, and
talks to C (and therefore every IoT SDK, RTOS and vendor HAL) without friction.

The rule that everything else serves:

**The language has a small number of concepts, each with one rule. Every feature is one of those
concepts, a library built on them, or a documented rewrite into them.** If a behavior cannot be
explained by the concept list plus the desugaring table, it is a bug in the design.

### No magic: the layering rule

```
syntax  --desugars into-->  lang  <--built on--  std  <--built on--  your code
```

- **`lang`** is a package written in ordinary Overt source. It declares, with real signatures and
  doc comments, every primitive the compiler provides: integer and pointer operations, the
  `Coroutine` interface, volatile and atomic access, reflection. A function declared without a
  body is provided from outside; in `lang` the compiler provides it, anywhere else the linker does
  (that is also how C functions are imported). There is no separate `@builtin` namespace, no
  magic identifiers, no special function names.
- **Syntax may only desugar into `lang`.** `a + b`, `&x`, `for`, `await`, `"text"`, `0..n` each map
  to a named `lang` item (appendix B). You can read what they mean by reading `lang`.
- **`std` is plain library code on top of `lang`.** `Option`, `Result`, allocators, `Mmio`,
  `Dyn`, executors, formatting, the OS layer, target startup code: all ordinary, all replaceable.
- **What is available to you is regular Overt.** There is nothing `std` can do that your code
  cannot.

### Guarantees

1. Nothing runs that you did not write or call: no destructors, no implicit fallible conversions,
   no hidden allocation, no unwinding, no global constructors, no auto-deref.
2. No undefined behavior outside `unsafe`. Inside `unsafe`, the list of UB is finite and
   enumerated (appendix C).
3. Semantics do not depend on optimization level. Debug and release builds compute the same thing
   and trap at the same points. The optimizer removes a check only when it proves it cannot fire.
4. Compile time and run time share one semantics, including target layout and integer ranges.

### Non-goals

- Maximal type-system expressiveness. No higher-kinded types, no type-level programming beyond
  what `comptime` gives for free.
- User-extensible syntax. Extension happens through `comptime` and libraries, never through new
  grammar, so every program reads with the same grammar.
- Hiding the machine.

---

## 1. The concept list

| # | Concept | One-line rule |
|---|---------|---------------|
| 1 | **Everything is a binding** | Every declaration is `const name = expr` or `var name = expr`. Files, directories and packages are structs whose fields are their declarations. |
| 2 | **Types are comptime values** | A type is a value of type `type`. Generics, interfaces, reflection, attributes and conditional compilation are ordinary compile-time computation. |
| 3 | **Integers are ranges** | Every integer type is `Int(lo..hi)`. Arithmetic is exact; only narrowing can fail, and narrowing is explicit. |
| 4 | **Places have states** | Every storage location is `uninit` or `init`. Moves, initialization, out-parameters, linear resources and `defer` are transitions of this one state machine. |
| 5 | **Borrows are scoped** | A borrow cannot outlive what created it; a returned borrow is tied to all borrowed arguments. There are no lifetime annotations. |
| 6 | **Capabilities are values** | Allocation, hardware, the OS, interior mutability and the program entry itself are values you hold. There is no ambient authority. |
| 7 | **Functions can suspend** | A function containing `yield` returns a sized frame value. Iterators, generators, async tasks and state machines are all this one thing. |

---

## 2. Syntax

### 2.1 Keywords

```
const var fn pub                                  bindings
struct enum interface impl                        type literals, conformance
if else match is for in break continue return     control
try await yield defer                             propagation, suspension, cleanup
comptime unsafe and or                            modifiers, logic
mut out take                                      borrow modes (only after `&`)
```

28 total, of which 3 are contextual. For comparison: Go 25, Rust 39, C 44, Zig 49.

What is *not* a keyword, and where it went instead:

| Elsewhere a keyword | In Overt |
|---------------------|----------|
| `while`, `loop` | `for cond { }`, `for { }` (one loop form) |
| `static` | a top-level `const` or `var` |
| `extern` | a function declared without a body |
| `as` | methods on integers: `x.to(u8)`, `x.wrap(u8)`, `x.sat(u8)`, `x.try_to(u8)` |
| `packed`, `align`, `callconv`, `export`, `section`, `volatile` | attributes (section 3.4) or `lang` functions |
| `import`, `use` | packages are bindings in scope; modules are fields of packages |
| `true`, `false`, `null`, `Self`, `type` | ordinary names bound in `lang` or by the code itself |
| `async` | a function whose body contains `yield` or `await` |
| `errdefer` | `defer` plus place states (section 6.3) |

### 2.2 One meaning per symbol

Symbols mean the same thing wherever they appear, and **type syntax mirrors value syntax**.

| Symbol | Meaning, everywhere |
|--------|---------------------|
| `x: T` | "x has type T": bindings, params, fields, captures |
| `=` | bind or assign; `.f = v` in a literal assigns field `f` of the value being built |
| `.name` | "`name` in the namespace of the expected type": `.some(x)`, `.output`, `.{ ... }`, `.ms(500)` |
| `.*` | dereference (postfix, chains left to right) |
| `&` | borrow: `&x` is a value, `&T` is its type; same for `&mut`, `&out`, `&take` |
| `[ ]` | sequences: `[N]T` / `[a, b, c]`; indexing `a[i]`; capture lists `fn [a, &mut b]` |
| `( )` | calls and grouping; type constructors are calls: `Int(0..8)`, `List(u8)` |
| `..`, `..=` | ranges: `0..n` is a value, `Int(0..n)` is the type of its elements |
| `?T` | optional; mirrors `.some(x)` / `.none` |
| `#[ ]` | attributes: comptime values attached to the next declaration or type |
| `->` | "returns": function types and function literals |
| `=>` | "then": match arms |

Patterns mirror constructors: an enum declared `enum { none, some(T) }` is built with `.some(x)`
and matched with `.some(x)`. Struct literals `.{ .x = 1 }` destructure as `.{ .x = a }`.

### 2.3 Shape of a program

```ov
const Point = struct { x: i32, y: i32 };

const mid = fn(a: u32, b: u32) -> u32 {
    (a + b) / 2               // Int(0..=2^33-2) / 2 is Int(0..=2^32-1), which is u32: no check
};

fn norm1(p: Point) -> u64 {   // sugar for `const norm1 = fn(p: Point) -> u64 { ... }`
    p.x.abs() + p.y.abs()     // Int(0..=2^32): fits u64, widens implicitly
}
```

Returning `-> u32` from `norm1` would be a compile error, because 2^32 does not fit. The type
checker catches the overflow bug that C would ship.

- Expression-oriented. A block's value is its last expression without a trailing `;`.
- Declarations are order-independent and resolved lazily, so recursion and self-reference need no
  forward declarations (`const Node = struct { next: ?Own(Node) }` just works).
- No shadowing. Evaluation order is left to right, always.
- `pub` is the only visibility modifier.

### 2.4 Modules and packages

A file is a struct type whose fields are its top-level declarations. A directory is a struct whose
fields are its files and subdirectories. A package is its root directory. The build declares which
packages a package depends on, and **each dependency is a binding in scope under its declared
name**. That is the whole module system:

```ov
const uart = mydriver.serial.uart;   // file mydriver/serial/uart.ov of package `mydriver`
pub const Baud = uart.Baud;          // re-export is just a binding
```

The prelude (names like `u8`, `bool`, `Option`) is `std.prelude`, and a build can swap it for
another one, or for none.

---

## 3. Types as comptime values

```ov
fn Pair(comptime T: type) -> type {
    struct { a: T, b: T }
}
const P = Pair(u16);   // same arguments, same type
```

- Generic functions are functions with `comptime` parameters.
- Conditional compilation is `if` on a comptime value:
  `if target.arch == .thumbv7em { ... }`, where `target` is a comptime value the build provides.
  The untaken branch is parsed and name-resolved but not type-checked.
- `comptime for` unrolls over a comptime sequence.
- Comptime execution is deterministic and sandboxed. Its only input is files of the current
  package, read with `std.meta.embed("path")`. That is enough to parse an SVD file into register
  types at compile time without a code generation step.
- Comptime uses the *target's* layout, word size and endianness.

### 3.1 Reflection

`std.meta` (on top of `lang.meta`) turns types into plain data and back:

```ov
const info = std.meta.info(Point);        // plain struct: kind, fields, size, align, attributes
const Point3 = std.meta.build(info.with_field("z", i32));
std.meta.size(Point)                      // 8
```

There is no separate macro language and there are no textual macros.

### 3.2 Interfaces

An interface is a type literal describing a set of types. Used where a type is expected, it
constrains a comptime parameter.

```ov
pub const Writer = interface {
    const Error: type;
    fn write(self: &mut Self, bytes: &[u8]) -> Result(usize, Self.Error);
};

impl Writer for Uart {
    const Error = UartError;
    fn write(self: &mut Uart, bytes: &[u8]) -> Result(usize, UartError) { ... }
}

fn log(comptime W: Writer, w: &mut W, msg: &[u8]) -> Result((), W.Error) {
    _ = try w.write(msg);
    .ok(())
}
```

- Inside an `interface` literal, `Self` names the conforming type. It is a parameter of the
  interface, not a keyword.
- Conformance is explicit. An `impl` lives in the module of the type or of the interface.
- `impl` members join the type's namespace; a collision is an error at the `impl`.
- An `impl` is a comptime value (the *witness*): a struct holding the conforming functions and
  constants. `std.meta.witness(Writer, Uart)` returns it.
- Inside a generic body, `w: W` with `W: Writer` may use only `Writer`'s members.
- Dynamic dispatch is library code: `std.Dyn(Writer)` reflects over the witness to build a vtable
  struct. The compiler knows nothing about it.

### 3.3 Operators are interfaces

Every operator desugars to a `lang.ops` interface: `a + b` is `lang.ops.Add.add(a, b)`, `a[i]` is
`lang.ops.Index.at(&a, i)`, `a == b` is `lang.ops.Eq.eq(&a, &b)`. Integers and floats are types
whose `impl`s the compiler provides (bodiless functions in `lang`). User types get operators the
same way, statically dispatched.

### 3.4 Attributes

An attribute is any comptime value, attached with `#[...]`:

```ov
#[link.section(".vectors"), link.keep]
pub const vectors = ...;

#[layout.bits(u32)]
const Ctrl = struct { enable: bool, mode: u2, _: u29 };

#[abi.c]
fn puts(s: *u8) -> i32;           // bodiless: provided by the linker
```

`link`, `layout`, `abi`, `panic` and `stack` are ordinary modules of `lang`, re-exported by the
prelude. The compiler acts on attributes whose types come from `lang`. Every other attribute is data
your own comptime code reads through `std.meta.attrs(decl)`, for example to rename fields in a
serializer. One mechanism, not one per feature.

---

## 4. Integers are ranges

Every integer type is `Int(lo..hi)` or `Int(lo..=hi)`. The familiar names are prelude bindings:

```ov
const u8    = Int(0..=255);
const i32   = Int(-2147483648..=2147483647);
const u3    = Int(0..=7);                    // arbitrary widths are not special
const usize = Int(0..=target.usize_max);
```

A literal `5` has type `Int(5..=5)`. There is no separate compile-time integer type: a literal is
simply the narrowest range.

### Rules

1. **Arithmetic is exact.** `a + b` has the range computed by interval arithmetic on the operand
   types. `+`, `-`, `*` never overflow. `(a + b) / 2` is correct for every `u32`.
2. **Widening is implicit**: `Int(R)` is accepted where `Int(S)` is expected iff `R` is within `S`.
   This cannot lose information, so it is not a conversion.
3. **Narrowing is explicit**, with four methods defined in `std` over `lang`:
   `x.to(T)` (traps if out of range), `x.wrap(T)` (modular, power-of-two ranges only),
   `x.sat(T)` (clamps), `x.try_to(T)` (returns `?T`).
4. **Typing is not flow-sensitive.** `if i < n` does not change the type of `i`. The optimizer may
   use the fact; the type checker never does, so types never depend on the optimizer.
5. **Width cap.** An intermediate range needing more than 128 bits is a compile error; narrow an
   intermediate. Comptime-only values are unbounded.
6. **Division** by a divisor whose range contains 0 is a trap site. If the range excludes 0
   (`Int(1..)`, a literal, a comptime value), it is not. Non-zero types come for free.
7. **Storage**: the smallest of 8/16/32/64/128 bits that holds the range directly. In a
   `layout.bits` struct, the minimal bit count. Unused bit patterns are niches (section 6.5).
8. **Bit operations** exist on `uN` types only and preserve the type. A shift amount whose range
   exceeds `N-1` is a trap site.

Compound assignment is sugar: `x += y` is `x = (x + y).to(TypeOf(x))`; `x +%= y` uses `.wrap`,
`x +|= y` uses `.sat`. So the trap sites in integer code are exactly: `.to`, a compound
assignment, a division by a possibly-zero value, and an index not proven in range.

```ov
const N = 64;
const Index = Int(0..N);

var head: Index = 0;
head = (head + 1) % N;    // Int(1..=64) % 64 is Int(0..64): no check
buf[head] = byte;         // buf: [N]u8, head: Index: no bounds check
```

Floats: `f16`, `f32`, `f64`, IEEE 754, no fast-math, no implicit int/float conversion.

---

## 5. Composite types

| Form | Meaning |
|------|---------|
| `struct { a: A, b: B }` | product, declaration order |
| `enum { a, b }` | tags only |
| `enum { none, some(T) }` | sum type with payloads |
| `[N]T` | array, a value |
| `&[T]`, `&mut [T]` | slice borrows (pointer and length) |
| `*T`, `?*T` | raw pointer, non-null / nullable |
| `fn(A, B) -> R` | function type |
| `.{ a, b }` / `.{ .x = a }` | anonymous tuple / struct literal |

`bool`, `Option`, `Result`, `Step`, `never` (the empty enum) and `()` are defined in `lang` as
ordinary enums and tuples. `?T` is sugar for `Option(T)`. Enum variants with payloads are
functions: `Result(u8, E).err` has type `fn(E) -> Result(u8, E)`.

`match` is exhaustive. `if e is p { a } else { b }` is sugar for a two-arm `match`.

```ov
match cmd {
    .read(r) => handle_read(r),
    .write(w) if w.len > 0 => handle_write(w),
    .write(_) => {},
    .reset => reset(),
}
```

---

## 6. Memory: kinds, places, layout

### 6.1 Kinds

Every type has three properties, computed structurally from its fields, and overridable only in the
restricting direction (`#[kind.linear]`, `#[kind.fixed]`):

| Property | Values | Meaning |
|----------|--------|---------|
| duplication | **copy** / **linear** | copy values may be duplicated with `=`; linear values must be consumed exactly once |
| region | **free** / **scoped** | scoped values contain a borrow and obey the scoping rule (section 7) |
| address | **movable** / **fixed** | fixed values never change address after construction |

- Integers, floats, raw pointers, `&T` and arrays of copy types are copy.
- `Own(T)`, files, DMA transfers, peripheral tokens and coroutine frames are linear.
- Coroutine frames, intrusive list nodes and buffers under DMA are fixed. A fixed value is
  constructed in place (section 6.5) and can be borrowed, or consumed in place with `&take`, but
  never moved.

There are no `Drop`, `Clone`, `Send`, `Sync`, `Unpin` or `Sized` special traits. Those problems are
solved by the three properties above and by capabilities (section 9).

### 6.2 Place states and borrow modes

Every place is `uninit` or `init`. The four borrow modes are named by the state transition they
perform on the place they borrow:

| Borrow | Before | After | Access |
|--------|--------|-------|--------|
| `&x` | init | init | shared read; many may coexist |
| `&mut x` | init | init | exclusive read-write |
| `&out x` | uninit | init | exclusive write; callee must initialize |
| `&take x` | init | uninit | exclusive; callee must consume the value in place |

Passing by value moves: a linear value leaves its place `uninit`, a copy value leaves it `init`.

Rules, all checked on the control-flow graph:

- Reading requires `init`. `var buf: [4096]u8;` is legal and free; reading it before writing is a
  compile error. There is no `undefined` value.
- **At every scope exit, every place holding a linear value must be `uninit`.** Forgetting to
  close, free, release or finish something is a compile error. No destructor is ever called
  implicitly.
- A place that is `init` on one path and `uninit` on another cannot be used after the join.

### 6.3 `defer`

`defer s` runs `s` at every exit of the enclosing block, in reverse order. If `s` consumes a
linear place, it runs on exactly the exits where that place is still `init` and is skipped on the
exits where the value was already moved. Which exits those are is known statically, so there is no
runtime flag. A place whose state is ambiguous at an exit is a compile error. This removes the need
for `errdefer`:

```ov
fn load(fs: &mut Fs, a: &mut Alloc, path: &[u8]) -> Result(Own(Config), Error) {
    const f = try fs.open(path) else Error.fs;
    defer fs.close(f);                  // every exit
    const cfg = try a.create(Config) else Error.oom;
    defer a.destroy(cfg);               // only exits where cfg is still owned
    try parse(&f, &out cfg.*) else Error.parse;
    .ok(cfg)                            // moves cfg out: the destroy is skipped on this exit
}
```

### 6.4 Pointers

- `Own(T)`: unique owning pointer, linear, safe to dereference. Produced by an allocator and given
  back to one; it does not remember which. Freeing requires holding the allocator.
- `*T`: raw pointer, copy, free, non-null. Making one is safe; dereferencing, offsetting and
  casting are `unsafe`.
- Volatile and atomic access are operations in `lang.mem` (`volatile_load`, `atomic_rmw`, ...), not
  type qualifiers. `std.mmio.Reg(T)` and `std.sync.Atomic(T)` wrap them.

### 6.5 Layout

Defined, never left to the implementation:

- `struct`: declaration order, natural alignment, C-style padding. Never reordered; a lint
  reports padding and suggests an order.
- `#[layout.bits(uN)] struct`: bit-exact, LSB first, fields must sum to N.
- `#[layout.c] struct`: C ABI layout, for FFI.
- `#[layout.tag(uN)] enum`: explicit tag representation.
- `#[layout.align(N)]`: raise alignment.
- **Niche rule** (one rule everywhere): an enum with one payload-free variant and one payload
  variant whose type has an unused bit pattern stores the tag in that pattern. `?*T` is one
  pointer; `?Int(0..255)` in a 16-bit slot uses 256.
- **Results are constructed in place.** A returned value is written directly into the caller's
  destination. This is a guarantee, not an optimization: returning a 4 KiB struct never copies it,
  and it is what makes fixed types constructible.

---

## 7. Borrows are scoped

A scoped value (a borrow, or anything containing one) may live in locals, parameters and fields of
other scoped types. It may be returned only from a function taking at least one borrow, and it is
then tied to **all** of that call's borrowed arguments.

```ov
fn longest(a: &[u8], b: &[u8]) -> &[u8] { if a.len >= b.len { a } else { b } }

const Parser = struct { input: &[u8], pos: usize };   // scoped
fn parser(input: &[u8]) -> Parser { .{ .input = input, .pos = 0 } }
```

You cannot store a borrow in a free value (a top-level `var`, an `Own`, a heap node). Long-lived
structures use ownership, indices into pools and arenas, or raw pointers in `unsafe` code.

**Static region.** Borrows of top-level `const`s have type `&static T`, which is free and copy and
is accepted wherever `&T` is expected. This is the only subtyping rule. String literals are
`&static [N]u8`. There are exactly two regions: static and scoped.

Exclusivity is checked per function with non-lexical liveness. Since borrows only escape through
returns tied to arguments, no interprocedural analysis is needed.

### 7.1 Method calls (decided)

**`.` operates on places; call arguments are values.**

`a.f` is a projection of the place `a`, whether `f` is a field or a function. So `a.f(b)` passes
the place `a` in whatever mode `f`'s first parameter asks for (value, `&`, `&mut`, `&out`,
`&take`), just as `a.x` reads a field without copying `a`. Arguments inside `( )` are ordinary
expressions, so borrowing them is written out: `f(&mut x)`.

Rationale: this is one rule, not an exception. The receiver is visibly a place (a path before `.`),
and mutating it still requires `var`. Marking receivers (`&mut v.push(1)`) would make every method
chain noisy and give `&` two parse meanings. Method lookup is the namespace of `TypeOf(a)`, then
its `impl`s. If `a: &mut T`, lookup goes through to `T`, because a borrow type has no namespace of
its own. There is no auto-deref beyond that one step and no `Deref` trait.

### 7.2 Closures

A closure names its captures, written exactly as they would be passed:

```ov
var count: u32 = 0;
const inc = fn [&mut count] (n: u8) { count += n };   // scoped: captures a borrow
const dbl = fn [x] () -> u32 { x * 2 };                // free and copy: x captured by value
```

A closure is sugar for an anonymous struct of its captures plus an `impl lang.ops.Call`. Its kinds
follow from its captures like any struct's. A closure with no captures is a plain function pointer.

---

## 8. Functions can suspend

One primitive covers iterators, generators, async I/O and hand-written state machines.

```ov
// in lang
pub const Step = fn(comptime Y: type, comptime R: type) -> type { enum { yielded(Y), done(R) } };

pub const Coroutine = fn(comptime In: type, comptime Y: type, comptime R: type) -> type {
    interface {
        fn resume(self: &mut Self, input: In) -> Step(Y, R);
        fn end(self: &take Self);   // finished: free; suspended: runs the pending defers
    }
};
```

- A function whose body contains `yield` or `await` is a coroutine. Its declared return type is an
  interface (`Coroutine(...)` or an alias of it). Calling it runs nothing: it constructs the frame
  in place and returns it.
- The frame is an ordinary value of an anonymous type, reachable as `f.Frame`. Its size is known at
  compile time: `std.meta.size(blink.Frame)`. Frames can live on the stack, in a top-level `var`,
  in a pool or in an `Own`. None of these needs a heap.
- Frames are **linear**: you must `end` them, which runs the defers of the current suspension
  point, so cancellation is explicit and structured. They are also **fixed**: locals may be
  borrowed across a `yield`, so the frame never moves. No `Pin` is needed because "fixed" is a
  kind, not a wrapper type.
- `yield v` suspends with `v`; its value is the next resume input.
- `await e` delegates to the coroutine `e` until it finishes, forwarding every yield outward and
  every input inward. The frame keeps the most recent resume input for this purpose (appendix B
  gives the exact rewrite). Awaiting works for any coroutine with matching `In` and `Y`, so it is
  not specific to async.
- A coroutine that awaits itself, directly or indirectly, has an infinitely large frame. That is a
  compile error, unless the recursion goes through an `Own`.

Built on it, in `std`:

```ov
pub const Iter  = fn(comptime T: type) -> type { Coroutine((), T, ()) };
pub const Async = fn(comptime R: type) -> type { Coroutine(&task.Waker, task.Pending, R) };
```

`for x in e` iterates any `Iter`: slices, ranges and generator functions alike.

```ov
fn evens(limit: u32) -> Iter(u32) {
    for i in 0..limit { if i % 2 == 0 { yield i; } }
}

fn blink(led: &mut Led, timer: &Timer) -> Async(never) {
    for {
        led.toggle();
        await timer.after(.ms(500));
    }
}
```

Executors (single-threaded, interrupt-driven, thread pool), wakers, timers and channels are `std`
library code with no compiler support. Since frames have static sizes, an embedded program's total
task memory is known at build time.

---

## 9. Capabilities are values

| Capability | Where it comes from | Grants |
|------------|---------------------|--------|
| `&mut A`, `A: Allocator` | you construct one (`FixedBuffer.init(&mut buf)`, `os.Heap`) | allocation |
| `&Critical` | `std.sync.critical(fn [...] (cs: &Critical) { ... })` | access to `Shared(T)` |
| peripheral tokens | the `Peripherals` value passed to `main` | MMIO |
| `&mut os.System` | the hosted `main` | files, sockets, threads, clocks, env |
| `&mut Build` | the build script | declaring the build graph |

There is no global allocator, no ambient `stdout`, and no singleton `take()` checked at run time.
Uniqueness of hardware is a type fact: startup code builds `Peripherals` exactly once and passes it
to `main`.

### 9.1 Interior mutability is always synchronized

Safe interior mutability exists only through synchronized primitives: `Atomic(T)`, `Mutex(T)`, and
`Shared(T)` accessed with a `&Critical`. A critical section is global mutual exclusion: interrupts
masked on single-core targets, plus a spinlock on multi-core ones. As a consequence, **every type
that is safe to share is safe to share across threads and interrupts**, so no `Send`/`Sync`
properties are needed. The unsynchronized `lang.mem.Cell(T)` is `unsafe`-only.

```ov
const ticks: Shared(u32) = .init(0);

#[panic.never]
fn on_systick() {
    std.sync.critical(fn [] (cs: &Critical) { ticks.get(cs).* +%= 1 });
}
```

`ticks.get(cs)` returns `&mut u32` tied to `cs` by the scoping rule, so it cannot escape the
critical section.

### 9.2 The program is a value

The build names one declaration as the program. Its type comes from whatever runtime package you
use, and it describes everything the compiler and linker need from you:

```ov
// hosted
pub const program = std.os.Program{ .main = main };
fn main(sys: &mut std.os.System) -> std.os.Exit { ... }

// bare metal
pub const program = stm32f4.Program{
    .main = main,
    .panic = stm32f4.halt,
    .interrupts = .{ .systick = on_systick, .tim2 = stm32f4.time.on_tim2 },
};
fn main(p: stm32f4.Peripherals) -> never { ... }
```

`stm32f4.Program` is a comptime function in an ordinary package. It generates the vector table (a
`#[link.section(".vectors")]` constant), the reset handler (copy `.data`, zero `.bss`, build
`Peripherals`, call `main`), and the interrupt-ABI wrappers. There are no magic entry names, no
`#[entry]`, and no `#[interrupt]`. Porting to a new chip means writing a package.

### 9.3 Errors and traps

- **Recoverable errors** are `Result(T, E)` values. `try e` returns early on `.err`. `try e else f`
  maps the error through `f` first. There are no implicit error conversions.
- **Traps** (a failed `.to`, a zero divisor, an unproven index, unwrapping `none`,
  `std.panic(...)`) call the handler supplied in the program value. There is no unwinding. A trap
  ends its execution context.
- **`#[panic.never]`** on a function is checked transitively: it may contain no undischarged trap
  site and may only call `#[panic.never]` functions.
- `overt check --traps` lists every remaining trap site and why it remains.

### 9.4 Function types and resource bounds

A function type carries its ABI (`#[abi.c]`, `#[abi.interrupt]`, `#[abi.naked]`), its panic
property and an optional stack bound (`#[stack.max(512)]`). All three are checked. Function
pointers carry them too, so the analyses survive indirect calls.

The compiler computes the worst-case stack use of every function from the whole-program call
graph. Recursion is rejected under a `stack.max` bound unless its depth is comptime-bounded
(`#[stack.depth(k)]`).

### 9.5 Foreign code

- A bodiless function with `#[abi.c]` is a C import; calling it is `unsafe`.
- `#[link.export("name")]` exports an Overt function or constant under a C symbol.
- `overt bind foo.h` (a tool, not a language feature) generates Overt declarations from C headers
  through libclang. `overt header` emits a C header for an Overt library. Together with static
  library output, this lets Overt code live inside ESP-IDF, Zephyr, FreeRTOS or vendor CMake
  projects, and lets C live inside Overt ones.
- Inline assembly is `lang.arch.asm(...)`, a `lang` function taking the template and operands as
  comptime data. It is `unsafe`.

---

## 10. Control flow

- `if`, `match`, `for`, `break`, `continue`, `return`, and optional labels
  (`'outer: for { break 'outer v; }`). `for` and `break` carry values.
- `for { }` loops forever, `for cond { }` loops while `cond` holds, `for p in e { }` iterates.
- `and` and `or` short-circuit. `!` negates.
- `for p in e { body }` rewrites to a `resume`/`end` loop over `e` (appendix B), so iteration is
  coroutines all the way down. The optimizer flattens simple iterators to plain loops.

---

## 11. Build

`build.ov` is ordinary Overt, evaluated at comptime with a `&mut Build` capability. It produces a
build graph, which the driver executes. Comptime cannot run programs; build steps can, and they are
declared explicitly as graph nodes (C compilers, `overt bind`, code signing).

```ov
pub fn build(b: &mut Build) {
    const fw = b.program(.{
        .root = "src/main.ov",
        .program = "program",
        .target = targets.thumbv7em_none_eabihf,
        .deps = .{ .stm32f4 = b.dep("stm32f4") },
        .memory = .{
            .flash = .{ .origin = 0x0800_0000, .len = 512 * 1024 },
            .ram   = .{ .origin = 0x2000_0000, .len = 128 * 1024 },
        },
    });
    b.test_on(fw, .qemu("lm3s6965"));
}
```

- Backend, linker and toolchain are build settings with per-target defaults. The language never
  fixes them (section 12).
- `test "name" { ... }` blocks run in the reference interpreter by default, natively, or on the
  target (QEMU, probe) on request.
- Packages are source-only and pinned by content hash.

---

## 12. Implementation architecture

Written in Rust. Designed as libraries with narrow interfaces, so that every stage past the
frontend can be swapped: backends, linkers, targets, analyses, evaluators and diagnostic renderers.

### 12.1 Pipeline

```
 source
   |  overt-syntax      lossless CST, error-recovering parser
   v
 HIR                   desugaring into `lang` items (appendix B), name resolution
   |  overt-sema        demand-driven: type checking and comptime evaluation interleaved
   v
 MIR                   control-flow graph over places; one body per instantiation
   |  MIR passes        checks: place states, kinds, borrows, scoping, panic.never, stack
   |                    transforms: coroutine lowering, defer lowering, range-based check elision
   |
   +--> overt-interp    reference interpreter: runs comptime and tests, defines the semantics
   |
   v
 LIR                   target-independent SSA: scalars, pointers, memory ops, calls, blocks
   |  LIR passes         ABI lowering (per backend capabilities), layout materialization
   v
 Backend  (trait)      C | QBE | Cranelift | LLVM | external process
   v
 Artifacts             object, assembly, C source, or in-memory code
   |  Tool steps        assembler, C compiler, when the backend emits text
   v
 Linker   (trait)      wild | mold | lld | GNU ld | MSVC link | none
```

### 12.2 Why two IRs

- **MIR** keeps what the checks need: places, borrow modes, enums, checked operations, suspension
  points. Place-state and borrow checking are dataflow problems on its CFG, so they are
  non-lexical and handle every kind of control flow the same way (`defer` and `yield` become
  explicit edges before checking).
- **LIR** is what every backend has in common: SSA values of fixed scalar types (`i8` to `i128`,
  `f32`, `f64`, `ptr`), loads and stores, calls, branches. Ranges, enums and aggregates have been
  lowered by then. A new backend translates LIR and nothing else. QBE's IL and Cranelift's CLIF are
  both close to it, so those translations are small.
- Both IRs have a **stable textual format**. Tests are written against it, dumps are readable, and
  an external backend can be any executable that reads LIR text on stdin.

### 12.3 Extension points

```rust
/// Code generation. Backends declare what they can do; the driver adapts the LIR to them.
pub trait Backend {
    fn id(&self) -> &str;
    fn supports(&self, target: &TargetSpec) -> Support;
    fn caps(&self) -> BackendCaps; // emits: Object | Asm | Source | Jit; lowers_c_abi; debug_info
    fn emit(&self, unit: &lir::Unit, target: &TargetSpec, out: &mut dyn ArtifactSink)
        -> Result<(), Diagnostics>;
}

/// Linking. Receives a target-independent plan: objects, sections, memory regions, symbols, entry.
pub trait Linker {
    fn id(&self) -> &str;
    fn supports(&self, target: &TargetSpec, plan: &LinkPlan) -> Support;
    fn link(&self, plan: &LinkPlan, out: &mut dyn ArtifactSink) -> Result<(), Diagnostics>;
}

/// External tools (assembler, C compiler) used by text-emitting backends and build steps.
pub trait Tool {
    fn id(&self) -> &str;
    fn run(&self, inv: &Invocation) -> Result<Output, Diagnostics>;
}

/// Analyses and transforms over MIR or LIR, with declared dependencies.
pub trait Pass<Ir> {
    fn id(&self) -> &str;
    fn requires(&self) -> &[&str];
    fn run(&self, body: &mut Ir, cx: &PassCx) -> PassResult;
}

/// Comptime and test evaluation. The interpreter is the reference; a JIT can sit behind this.
pub trait Evaluator {
    fn call(&mut self, f: FnRef, args: &[Value]) -> Result<Value, Trap>;
}

/// Diagnostics are data; renderers for terminal, JSON and LSP.
pub trait DiagSink {
    fn emit(&mut self, d: Diagnostic);
}
```

- **Targets are data, written in Overt.** A `TargetSpec` (word size, endianness, alignments, ABIs,
  features, object format, default backend and linker) is a comptime value from the `targets`
  package. Adding a target means adding a file; the compiler is not rebuilt.
- **Registration is static by default** (Cargo features choose which backends and linkers are
  compiled in) and **dynamic through processes** (external backends and linkers speak LIR text and
  `LinkPlan` JSON). Rust has no stable dylib ABI, and processes give isolation for free.
- **The link plan is abstract.** Memory regions and sections declared in the build go into
  `LinkPlan`. The lld and GNU ld adapters turn them into a linker script; others use flags. No
  stage above the linker adapter knows linker-script syntax.
- **Nothing above LIR knows about any backend**, and nothing above the linker adapter knows about
  any linker.

### 12.4 Backends

| Backend | Role | Targets | Notes |
|---------|------|---------|-------|
| **C** | reach | anything with a C99 compiler | first to ship; covers Cortex-M, AVR, Xtensa, PIC and vendor toolchains; emits only defined C, with `-fwrapv -fno-strict-aliasing` as extra safety |
| **QBE** | small, fast, decent code | amd64, arm64, rv64 | subprocess fed LIR translated to QBE IL text; handles C ABI aggregates itself |
| **Cranelift** | fast debug builds, JIT | x86_64, aarch64, riscv64, s390x | in-process; later a candidate comptime JIT behind `Evaluator` |
| **LLVM** | peak optimization, widest native reach | ARM32/Thumb, RISC-V 32, AVR, wasm, ... | optional feature; not needed to build the compiler |

QBE and Cranelift have no 32-bit embedded targets, so microcontrollers go through C or LLVM. That
is a deployment detail the language knows nothing about.

### 12.5 Linkers

| Linker | Use |
|--------|-----|
| **wild** | fast incremental development links on Linux ELF (x86_64, aarch64, riscv64); check its linker-script coverage before using it for firmware |
| **mold** | fast ELF links, mature |
| **lld** | ELF / COFF / Mach-O / wasm with linker scripts; default for embedded |
| **GNU ld** | vendor-toolchain compatibility |
| **none** | emit objects or a static library for a foreign build system |

### 12.6 Workspace

```
compiler/
  base/   overt-span  overt-diag  overt-query (incremental, demand-driven engine)
  front/  overt-syntax  overt-hir  overt-sema
  mid/    overt-mir  overt-mir-passes  overt-interp
  low/    overt-lir  overt-lir-passes  overt-abi  overt-target
  back/   overt-backend (traits)  overt-backend-{c,qbe,cranelift,llvm,process}
  link/   overt-link (traits)  overt-link-{wild,mold,lld,gnu,msvc,process}
  tools/  overt-build  overt-driver (library)  overt-cli  overt-lsp  overt-fmt  overt-bind
lib/
  lang/     primitives: declarations only, bodies provided by the compiler
  std/      prelude, mem, sync, task, fmt, meta, mmio, os/*
  targets/  TargetSpec values
  boards/   example runtime packages (stm32f4, rp2040, esp32c3, linux, wasi)
```

Principles: `overt-driver` is a library and the CLI is a thin shell over it; the LSP and the test
harness use the same library. Crates depend only on crates in earlier groups. Every IR has a
verifier that runs between passes in debug builds of the compiler.

### 12.7 Semantics are executable

`overt-interp` interprets MIR under an explicit target model (layout, endianness, ranges, traps).
It runs comptime code and `test` blocks, and it defines what programs mean. Every backend is
checked against it by differential testing, using the test corpus and randomly generated programs
in the style of Csmith. That keeps guarantees 3 and 4 true across backends.

---

## 13. Examples

### 13.1 Registers

```ov
#[layout.tag(u2)]
const Mode = enum { input, output, alt, analog };

#[layout.bits(u32)]
const Moder = struct { pins: [16]Mode };

#[layout.c]
const Gpio = struct {
    moder:  Reg(Moder),
    otyper: Reg(u32),
    ospeed: Reg(u32),
    pupdr:  Reg(u32),
    idr:    Reg(u32),
    odr:    Reg(u32),
    bsrr:   Reg(#[layout.bits(u32)] struct { set: u16, reset: u16 }),
};

fn set_output(g: &mut Gpio, comptime pin: Int(0..16)) {
    g.moder.modify(fn [pin] (r: &mut Moder) { r.pins[pin] = .output });
}
```

`Reg(T).modify` is a volatile read, a call and a volatile write. `r.pins[pin]` is unchecked
because `pin: Int(0..16)` and `pins: [16]Mode`.

### 13.2 Async blinky with serial echo

```ov
pub const program = stm32f4.Program{
    .main = main,
    .panic = stm32f4.halt,
    .interrupts = .{ .tim2 = stm32f4.time.on_tim2, .usart2 = stm32f4.serial.on_usart2 },
};

fn main(p: stm32f4.Peripherals) -> never {
    var led = p.gpioa.pin(5).output();
    var timer = stm32f4.time.Timer.init(p.tim2);
    var serial = stm32f4.serial.Serial.init(p.usart2, .{ .baud = 115_200 });

    var a = blink(&mut led, &timer);   // frames built in place in main's stack frame
    var b = echo(&mut serial);
    std.task.run(.{ &mut a, &mut b })  // -> never; main never returns, so the frames live forever
}

fn blink(led: &mut Led, timer: &Timer) -> Async(never) {
    for {
        led.toggle();
        await timer.after(.ms(500));
    }
}

fn echo(s: &mut Serial) -> Async(never) {
    var buf: [64]u8;
    for {
        const n = await s.read(&out buf);   // borrow of a frame local held across a suspension
        await s.write(buf[0..n]);
    }
}
```

### 13.3 Hosted

```ov
pub const program = std.os.Program{ .main = main };

fn main(sys: &mut std.os.System) -> std.os.Exit {
    var out = sys.stdout();
    std.fmt.print(&mut out, "{} args\n", .{ sys.args().len });
    .ok
}
```

### 13.4 Ring buffer

```ov
pub fn RingBuffer(comptime T: type, comptime N: Int(1..=65536)) -> type {
    const Index = Int(0..N);
    const Ring = struct {
        buf: [N]T,
        head: Index = 0,
        len: Int(0..=N) = 0,

        pub fn push(self: &mut Ring, v: T) -> Result((), T) {
            if self.len == N { return .err(v); }
            self.buf[(self.head + self.len) % N] = v;   // Int(0..N): unchecked
            self.len = (self.len + 1).to(Int(0..=N));   // holds because of the `if` above
            .ok(())
        }

        pub fn pop(self: &mut Ring) -> ?T {
            if self.len == 0 { return .none; }
            const v = self.buf[self.head];
            self.head = (self.head + 1) % N;
            self.len = (self.len - 1).to(Int(0..=N));
            .some(v)
        }
    };
    Ring
}
```

### 13.5 Reflection

```ov
fn dump(comptime T: type, v: &T, w: &mut Dyn(Writer)) -> Result((), Writer.Error) {
    comptime for f in std.meta.info(T).fields {
        try std.fmt.print(w, "{} = {}\n", .{ f.name, std.meta.field(v, f.name) });
    }
    .ok(())
}
```

`std.fmt.print` parses its format string at comptime and checks it against the argument tuple.

---

## 14. Roadmap

| Phase | Deliverable | Exit criterion |
|-------|-------------|----------------|
| 0 | Core spec + corpus of ~40 programs (embedded, hosted, async, FFI) written before the compiler | Every program is explained by the concept list and appendix B alone |
| 1 | `syntax`, `hir`, `sema`, `interp`, MIR text format; `lang` and minimal `std` | Corpus type-checks; comptime generics, interfaces and reflection run |
| 2 | MIR checks: place states, kinds, defer, borrows, scoping; coroutine lowering | Must-fail corpus rejected with the expected diagnostic codes; generators and async run in `interp` |
| 3 | LIR, C backend, `Linker` trait with lld and GNU ld, QEMU runner | Blinky on a real board; differential tests against `interp` pass |
| 4 | QBE and Cranelift backends; wild and mold; hosted `std.os` | Compiler test suite passes natively on all three backends |
| 5 | Board packages, `std.task` executors, stack and `panic.never` analyses, `overt bind` | Measured stack high-water mark within the computed bound; Overt running inside an ESP-IDF project |
| 6 | `fmt`, LSP, `check --traps`, LLVM backend | Usable daily; code size within 10% of C on a reference firmware |

---

## 15. Decision log

| # | Decision | Status |
|---|----------|--------|
| 1 | Integers as ranges | accepted |
| 2 | Scoped borrows, no lifetime annotations | accepted |
| 3 | `.` operates on places: receivers passed per signature, arguments marked | decided (7.1) |
| 4 | Async is first-class through one suspension primitive shared with iterators | accepted (8) |
| 5 | No `@builtins`: primitives are bodiless declarations in the `lang` package | accepted |
| 6 | Pluggable backends and linkers; the language privileges none | accepted (12) |
| 7 | A `fixed` kind instead of `Pin` | proposed (6.1) |
| 8 | Safe interior mutability is always synchronized, so no `Send`/`Sync` | proposed (9.1) |
| 9 | `defer` plus place states replaces `errdefer` | proposed (6.3) |
| 10 | Single loop keyword `for`; no `while`/`loop` | proposed (10) |

### Open questions

1. **Borrow precision.** "Tied to all borrowed arguments" is conservative. Lean: keep it; add
   `&noescape` for a parameter only if the corpus shows real pain.
2. **Flow-sensitive narrowing.** Letting `if x < N` refine `x` would remove most `.to` calls, but
   types would then depend on control flow. Lean: no for v1; decide with corpus data.
3. **Single-threaded shared ownership.** Rule 9.1 means reference counting must be atomic. Lean:
   accept; arenas and handles cover most non-atomic uses.
4. **The implicit resume input.** `await` forwards the frame's last input, the one value the
   programmer cannot name directly. Lean: expose it only as the result of `yield`, and document the
   rewrite.
5. **Large implicit copies.** Lean: a lint with a configurable byte threshold, not a language rule.
6. **Integer width cap.** Lean: 128 bits everywhere, independent of target, so code stays portable.

---

## Appendix A: `lang` at a glance

`lang` is source code. Its modules:

| Module | Contents |
|--------|----------|
| `lang` | `type`, `Int`, `f16`/`f32`/`f64`, `bool`, `never`, `Range`, `Option`, `Result`, `Step`, `Coroutine` |
| `lang.ops` | `Add`, `Sub`, `Mul`, `Div`, `Rem`, `BitAnd`, ..., `Eq`, `Ord`, `Index`, `Call` |
| `lang.mem` | `Ref`, `RefMut`, `RefOut`, `RefTake`, `Ptr`, `volatile_load`/`volatile_store`, `atomic_*`, `Cell`, `copy`, `fill` |
| `lang.meta` | type info, witnesses, attributes, `TypeOf`, file embedding |
| `lang.kind` | `linear`, `fixed` |
| `lang.layout` | `bits`, `c`, `tag`, `align` |
| `lang.link` | `section`, `keep`, `export`, `symbol`, `weak` |
| `lang.abi` | `c`, `interrupt`, `naked`, `native` |
| `lang.panic` | `never`, `trap`, `unreachable` |
| `lang.stack` | `max`, `depth` |
| `lang.arch` | `asm`, and target-specific intrinsics under `lang.arch.<arch>` |

## Appendix B: desugaring table

| Syntax | Meaning |
|--------|---------|
| `fn f(...) ...` | `const f = fn(...) ...` |
| `?T` | `lang.Option(T)` |
| `&T`, `&mut T`, `&out T`, `&take T` | `lang.mem.Ref(T)`, `RefMut(T)`, `RefOut(T)`, `RefTake(T)` |
| `a..b`, `a..=b` | `lang.Range` values (half-open / closed) |
| `a + b` (every operator) | `lang.ops.Add.add(a, b)` (the corresponding interface) |
| `a[i]` | `lang.ops.Index.at(&a, i)` or `at_mut(&mut a, i)` by context |
| `x op= y` | `x = (x op y).to(lang.meta.TypeOf(x))` |
| `x op%= y` / `x op\|= y` | `x = (x op y).wrap(...)` / `.sat(...)` |
| `try e` | `match e { .ok(v) => v, .err(x) => return .err(x) }` |
| `try e else f` | `match e { .ok(v) => v, .err(x) => return .err(f(x)) }` |
| `if e is p { a } else { b }` | `match e { p => a, _ => b }` |
| `for c { b }` | `for { if !c { break; } b }` |
| `for p in e { b }` | `{ var it = e; defer it.end(); for { match it.resume(()) { .yielded(p) => b, .done(_) => break } } }` |
| `await e` | `{ var f = e; defer f.end(); for { match f.resume($in) { .done(v) => break v, .yielded(y) => $in = yield y } } }`, where `$in` is the frame's last resume input |
| `a.f(args)` | `TypeOf(a).f(<a in f's first-parameter mode>, args)` |
| `fn [caps] (ps) -> R { b }` | anonymous struct of `caps` plus `impl lang.ops.Call` |
| `"text"` | `&static [N]u8` in read-only data |
| `.name` | `E.name`, where `E` is the expected type |

## Appendix C: undefined behavior inside `unsafe`

1. Dereferencing a `*T` that does not point to a live, aligned, initialized `T`.
2. Writing through a `*T` while a borrow of an overlapping place is live.
3. Reinterpreting bits as a type for which they are invalid (out-of-range `Int`, bad enum tag).
4. Calling a foreign function in violation of its contract.
5. Inline assembly that violates its declared operands or clobbers.
6. Data races on non-atomic memory.
7. Moving a fixed value through raw memory operations while it is borrowed.
8. Reaching `lang.panic.unreachable`.

Safe code cannot trigger any of these. That is the definition of safe.
