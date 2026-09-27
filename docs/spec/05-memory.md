# 05. Memory

## 1. Kinds

Every type has three properties, computed structurally from its members. A declaration may make a
type more restrictive (`#[kind.linear]`, `#[kind.fixed]`, `#[kind.free]`), never less. The
attributes go on the type's literal, also when it is a constructor's value:
`Own := ($T: type) -> type { #[kind.linear] #[kind.free] struct { ... } }`.

| Property | Values | Meaning |
|----------|--------|---------|
| duplication | **copy** / **linear** | copy values are duplicated by `=`; linear values must be consumed exactly once |
| region | **free** / **scoped** | scoped values contain a borrow and obey the scoping rule (section 4) |
| address | **movable** / **fixed** | fixed values never change address after construction |

A fourth fact is derived, never declared: a type is **cell-free** if it contains no `lang.mem.Cell`.
It is used by the aliasing rules (section 7).

- Integers, floats, raw pointers, `&T` and arrays of copy types are copy.
- `Own(T)`, files, DMA transfers, peripheral tokens and coroutine frames are linear.
- Coroutine frames, intrusive list nodes and buffers under DMA are fixed. A fixed value is
  constructed in place and may be borrowed or consumed in place (`&take`), never moved.

There are no `Drop`, `Clone`, `Send`, `Sync`, `Unpin` or `Sized` traits. Their jobs are done by the
three properties and by capabilities (spec 07).

## 2. Place states and borrow modes

Every place is `uninit` or `init`. The four borrow modes are named by the transition they perform
on the place they borrow:

| Borrow | Before | After | Access |
|--------|--------|-------|--------|
| `&x` | init | init | shared read; any number may coexist |
| `&mut x` | init | init | exclusive read and write |
| `&out x` | uninit | init | exclusive write; the callee must initialize |
| `&take x` | init | uninit | exclusive; the callee must consume the value in place |

Passing by value moves. A linear value leaves its place `uninit`; a copy value leaves it `init`.

A parameter type may add `temp` to a borrow (`&temp T`, `&temp mut T`): the borrow is lent for the
call only (section 4). It changes no place state, so arguments are written as ordinary borrows.

Rules, all checked on the control-flow graph:

- Reading requires `init`. `mut buf: [4096]u8;` costs nothing, and reading it before writing it is a
  compile error. There is no `undefined` value.
- A part named without running the program (a field, a tuple position, an item at a constant
  index) has its own state once written: `buf[0] = 1;` makes `buf[0]` readable, not `buf[1]`. A
  part at an index known only at run time needs the whole place `init`.
- A linear value cannot be moved out of a place reached through a borrow's `.*`: a borrow does
  not own what it points to. Matching or reading through the borrow itself binds by borrow
  instead (`place.move_out_of_borrow`). An owner held by value owns its target, so moving a
  part out of it is allowed (`cur = node.next` before freeing `node`).
- Writing a whole linear place that still holds a value is a leak (`place.linear_leak`). Moving a
  linear value is an exclusive access: it conflicts with any live borrow of it.
- **At every scope exit, every place holding a linear value must be `uninit`.** Forgetting to close,
  free, release or finish something is a compile error. No destructor ever runs implicitly.
- A place that is `init` on one incoming path and `uninit` on another cannot be used after the join.
- **An aggregate literal that exits early** (a `?` in a field initializer, say) leaves the fields
  already built owned by the enclosing scope, and the rule above applies to them.

## 3. `defer`

`defer s;` runs `s` at every exit of the enclosing block, in reverse order. If `s` consumes a linear
place, it runs on exactly the exits where that place is still `init` and is skipped on exits where
the value was already moved. Which exits those are is known statically, so there is no runtime flag.
A place whose state is ambiguous at an exit is a compile error. This removes the need for
`errdefer`. A statement consuming several linear places runs only on exits where all of them are
held; on the others the ones still held must be consumed some other way. An exit inside a deferred
statement (a `?` in it) runs only the statements deferred before it.

```vun
load := (fs: &mut Fs, a: &mut Alloc, path: &[u8]) -> Result(Own(Config), Error) {
    f := fs.open(path).map_err(Error.fs)?;
    defer fs.close(f);                    // every exit
    cfg := a.create(Config).map_err(Error.oom)?;
    defer a.destroy(cfg);                 // only exits where cfg is still owned
    parse(&f, &out cfg.*).map_err(Error.parse)?;
    .ok(cfg)                              // cfg moves out: the destroy is skipped on this exit
};
```

## 4. Borrows are scoped

A scoped value (a borrow, or anything containing one) may live in locals, parameters and fields of
other scoped types. It may be returned only from a function taking at least one borrow, and it is
then tied to **all** of that call's borrowed arguments. That rule replaces lifetime annotations.

```vun
longest := (a: &[u8], b: &[u8]) -> &[u8] { if a.len() >= b.len() { a } else { b } };

Parser := struct { input: &[u8], pos: usize };           // scoped
parser := (input: &[u8]) -> Parser { .{ .input = input, .pos = 0 } };
```

**Temporary borrows.** A parameter whose type is `&temp T` or `&temp mut T` is lent for the call
only: the result is not tied to it, and the function may not return it or put it into anything it
returns. So `find := (hay: &[u8], needle: &temp [u8]) -> &[u8]` returns a borrow tied to `hay`
alone, and a caller may pass a short-lived `needle`. The table `borrows` records this as each
mode's extent (`tied` or `temp`). A `&temp` borrow and the ordinary borrow of the same transition
and access are accepted for each other, so the rule is carried by the parameter and checked in the
callee's body, where a `&temp` parameter is a loan that must not reach the result. A borrow
written through an exclusive borrow parameter outlives the call, so it may not be of the
function's own storage (`borrow.escape`).

A borrow cannot be stored in a free value: a top-level binding, an `Own`, a heap node. A type is
made free by `#[kind.free]`: neither its parts nor its type arguments may then be scoped, so
`Own(Parser)` for a scoped `Parser` is an error where such a value is built. Long-lived
structures use ownership, indices into pools and arenas, or raw pointers in `unsafe` code.

**Trees in arenas.** Because scoped values may live in fields of other scoped types, a tree can be
built from borrows of an arena: `std.mem.Arena` hands out `&mut T` tied to the arena's borrow, and
`Expr := enum { lit(i64), add(&Expr, &Expr) }` is an ordinary scoped type. The tree lives as long
as the arena borrow; a tree that must outlive its scope uses `Own` nodes and an allocator. For
graphs with cycles, `std.collections.Handle(T)` gives typed indices into a pool, over nominal
structs (spec 02), with checked indexing whose checks range analysis removes when it can.

**Static region.** Shared borrows of what lives as long as the program have type `&static T`, which
is free and copy and is accepted wherever `&T` is expected: an immutable top-level binding or a
part of one reached by fields and indices (`&table[i]`, `&config.name`), and a literal, or a
tuple, array or record of literals (`&[]`, `&.{ 1, 2 }`). It is the only subtyping
rule. A borrow of a top-level binding reaches its one storage, never a copy of its value; one
holding a cell (not cell-free) is written through it, so its storage is writable. String literals
are `&static [N]u8`. A `mut` top-level binding is a static that can change,
so its borrows are scoped, and at compile time it has no value: what it holds is known at run time
only. There are exactly two regions, static and scoped. In the table `borrows`, `static`
is the extent of the row `lang.mem.RefStatic`; borrows of one transition and access meet in the
shorter extent, so `if c { "text" } else { buf }` is a plain `&[u8]`.

Exclusivity is checked per function with non-lexical liveness. Borrows escape only through returns
tied to arguments, so no interprocedural analysis is needed.

## 5. Pointers

- `Own(T)`: unique owning pointer, linear, free, safe to dereference. It comes from an allocator
  and goes back to one; freeing it requires holding the allocator. It is a library struct whose
  pointer field is marked `#[kind.owns]`: `.*` on a struct reads through the field so marked,
  as it does on a borrow or pointer, and `.` looks through it as through a borrow when the owner
  lacks the member, so `cfg.name` reaches a field of what an `Own(Config)` owns.
- `*T`: raw pointer; copy, free, non-null. Creating one (`lang.mem.pointer_to`) and reading its
  address are safe. Dereferencing, offsetting and casting (`lang.mem.read`, `write`, `offset`,
  `cast`, `copy`, `fill`) are `unsafe`; `std.ptr` gives them as methods of `*T`.
- Volatile and atomic access are operations in `lang.mem`, not type qualifiers.
  `std.hw.Reg(T)` and `std.sync.Atomic(T)` wrap them.

## 6. Layout

Defined, never left to the implementation:

- `struct`: declaration order, natural alignment, C-style padding. The compiler never reorders; the
  `perf.padding` lint reports padding and offers a reordering fix.
- `#[layout.bits(uN)] struct`: bit-exact, least significant bit first; fields must sum to N bits.
- `#[layout.c] struct`: the target C ABI's layout, which is the default one on every target
  described, and a promise that each field is of a type C reads the same way: full-width
  integers, `bool`, `f32`, `f64`, `*T`, `?*T`, `#[abi.c]` functions and `?` of them, arrays of
  these and other `#[layout.c]` structs (`layout.not_c`).
- `#[layout.tag(uN)] enum`: explicit tag representation; `uN` has a pattern for each variant
  (`layout.tag`).
- `#[layout.align(N)]`, `#[layout.offset(N)]`: raise alignment, `N` a power of two
  (`layout.align`); place a field at an offset.
- **Niches (one rule).** An enum with one payload-free variant and one payload variant whose type has
  unused bit patterns stores the tag in an unused pattern. Unused patterns are: the null pointer,
  out-of-range values of range integers (`Int(0..200)` in 8 bits has 56), and unused enum tags.
  `?*T` is one pointer; `?Int(0..255)` is one byte, `.none` being 255; `?Int(0..256)` is two.
- **Which patterns.** The unused patterns of an integer are those after its range's last value,
  counted up and wrapping in its storage; of a pointer, null; of a tag, those past the last
  variant. A struct, tuple or array has the largest run among its parts, the first on a tie; an
  enum using the niche rule takes the first pattern of the run, keeping the rest. An enum tagged
  otherwise stores the variant's index first, in the smallest integer holding the indices (or the
  `#[layout.tag]` integer), and every payload after it at the enum's alignment.
- Struct-of-arrays is a library: `std.collections.Soa(T)` generates the split layout by reflection.

## 7. Construction, assignment and passing

These rules exist because the naive "results are always built in place" model failed in practice
(research notes, section 3: Zig's result location semantics, now slated for removal).

1. **Initialization is in place.** Initializing an `uninit` place (a binding, an `&out` target, a
   field in a literal, a frame in a coroutine) writes the result directly into it. This is a
   guarantee: returning a 4 KiB struct into a new binding never copies it, and it is what makes
   fixed types constructible.
2. **Assignment is evaluate, then move.** Storing into an `init` place evaluates the right-hand side
   completely, then moves it in. `p = .{ .x = p.y, .y = p.x }` swaps. The optimizer elides the
   temporary only when the right-hand side provably does not read the destination.
3. **Passing by value is a semantic copy or move.** The ABI may pass a hidden pointer for large
   values only when the source is a temporary, a moved linear value, or a local with no live
   borrow; this is never observable. Small `&T` of cell-free `T` may be passed in registers.
4. **Function addresses are not unique.** Identical lowered functions may be merged (spec 13).

## 8. Aliasing facts

Part of the language definition, not optimizer folklore:

- `&mut`, `&out` and `&take` borrows are exclusive: no other access to the place while they live.
  Backends may mark them `noalias` / `restrict`.
- `&T` of a cell-free `T` points to memory that does not change while the borrow lives. Backends
  may mark it read-only and non-aliasing for writes.
- If a function's result type is free, none of its borrow parameters escape (`captures(none)`).
- Violating these through raw pointers is undefined behavior (appendix C). The reference
  interpreter checks them, so `vun test` detects violations in `unsafe` code.
