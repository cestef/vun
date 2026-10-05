#import "../template.typ": *
#import "../figures.typ": place-states

= Places, kinds and borrows <sec-memory>

== Kinds

Every type has three structural properties, computed from its members. A declaration may make a
type more restrictive, with the attributes `kind.linear`, `kind.free` and `kind.fixed` on its
literal, but never less.

- *Duplication.* A type is _copy_ if all its members are copy and it is not declared linear;
  otherwise it is _linear_, and each of its values must be consumed exactly once.
- *Region.* A type is _scoped_ if it contains a borrow, and _free_ otherwise.
- *Address.* A type is _fixed_ if its values must not change address after construction, and
  _movable_ otherwise.

A fourth fact is derived and cannot be declared: a type is _cell-free_ if it contains no
unsynchronized interior-mutable cell (`lang.mem.Cell`). Integers, floating-point values, raw
pointers, shared borrows and arrays of copy types are copy. Owning pointers, files, DMA
transfers, peripheral tokens and coroutine frames are linear. Coroutine frames, intrusive list
nodes and buffers under DMA are fixed.

There are no destructor, clone, thread-safety, pinning or sizedness traits; their roles are
taken by these properties and by capabilities (@sec-capabilities). Linear types in the tradition
of Austral @austral make resource release an obligation checked by the compiler rather than an
implicit call.

== Place states

Every place, meaning a local, a field, an element or the target of a write-only borrow, is in one of
two states, $U$ (uninitialized) or $I$ (initialized). The borrow forms of the language are named by
the transition they perform on the place they borrow (@tab-places, @fig-states). The table is the `borrows`
table of `lang` shown in @sec-principles; the checker reads its columns rather than encoding the
forms.

#figure(
  table(
    columns: (auto, auto, auto, auto, 1fr),
    align: (left, center, center, left, left),
    toprule,
    [Form], [Before], [After], [Extent], [Access],
    midrule,
    [`&x`], [$I$], [$I$], [tied], [shared read; any number may coexist],
    [`&mut x`], [$I$], [$I$], [tied], [exclusive read and write],
    [`&out x`], [$U$], [$I$], [tied], [exclusive write; the callee must initialize],
    [`&take x`], [$I$], [$U$], [tied], [exclusive; the callee must consume in place],
    [`&temp`, `&temp mut`], [$I$], [$I$], [call], [shared or exclusive, lent for the call only],
    [`&static`], [$I$], [$I$], [program], [shared read of an immutable top-level binding],
    [move of a linear value], [$I$], [$U$], [], [ownership leaves the place],
    [copy], [$I$], [$I$], [], [a duplicate is made],
    bottomrule,
  ),
  caption: [Borrow forms and moves as transitions on place states.],
) <tab-places>

#figure(
  place-states,
  caption: [The two place states and the transitions of @tab-places. The exit rule constrains the
    state of linear places at scope exits.],
) <fig-states>

The checker is a forward dataflow analysis over the control-flow graph of each function, which
computes a state for every place at every program point. Parts named without running the program
(a field, a tuple position, an element at a constant index) have their own state once written, so
`buf[0] = 1;` makes `buf[0]` readable but not `buf[1]`; a part at an index known only at run time
requires the whole place to be initialized. The rules are few.

#rule[reads][Reading a place requires state $I$. An uninitialized declaration is legal and costs
nothing; reading it before a write is a compile-time error. No `undefined` value exists.]

#rule[exits][At every exit of a scope, every place of a linear type declared in that scope is in
state $U$.]

#rule[joins][A place whose state differs between the incoming edges of a join point is unusable
after the join, and violates the exit rule if it is linear.]

#rule[ownership through borrows][A linear value cannot be moved out of a place reached through a
borrow; matching or reading through the borrow binds by borrow instead.]

The exit rule replaces destructors. Forgetting to close a file, free an allocation or end a
transfer is a compile-time error, and cleanup is always a call written in the source. To keep this
practical, `defer s` schedules `s` at every exit of the enclosing block, with one refinement that
follows from place states: if `s` consumes a linear place, it runs exactly on the exits where that
place is in state $I$ and is omitted on the exits where the value was moved away. Because states
are computed statically, this needs no run-time flag, and a separate construct for cleanup on the
error path only is unnecessary. A place whose state at an exit is ambiguous is an error.

```vun
load := (fs: &mut Fs, a: &mut Alloc, path: &[u8]) -> Result(Own(Config), Error) {
    f := fs.open(path).map_err(Error.fs)?;
    defer fs.close(f);                    // every exit
    cfg := a.create(Config).map_err(Error.oom)?;
    defer a.destroy(cfg);                 // skipped on the exit where cfg moves out
    parse(&f, &out cfg.*).map_err(Error.parse)?;
    .ok(cfg)
};
```

== Scoped borrows

#rule[scoping][A value of scoped type may be stored in locals, parameters and fields of other scoped
types. It may be returned only by a function with at least one borrowed parameter, and is then tied
to all borrowed arguments of the call.]

The rule replaces lifetime annotations. It is conservative: a returned borrow keeps every borrowed
argument borrowed, including those it does not derive from. In exchange, borrows escape only
through returns tied to arguments, so exclusivity is checked per function with non-lexical
liveness, and no interprocedural analysis is required.

```vun
longest := (a: &[u8], b: &[u8]) -> &[u8] { if a.len() >= b.len() { a } else { b } };
find := (hay: &[u8], needle: &temp [u8]) -> &[u8] { ... };   // tied to hay only
```

Two refinements reduce the conservatism without adding annotations. A parameter of type `&temp T`
or `&temp mut T` is lent for the call only: the result is not tied to it, and the callee may not
return it or store it in anything it returns, which the checker verifies in the callee's body.
Borrows of immutable top-level bindings, and string literals, have type `&static T`, which is free
and copy and is accepted wherever a shared borrow is expected; borrows of one form meet in the
shorter extent, so `if c { "text" } else { buf }` is a plain `&[u8]`. Borrows of one transition serve for one
another when they leave others no more access: `&static T` and `&mut T` where `&T` is expected.
This is the language's only subtyping rule. There are exactly two regions, static and scoped.

A borrow cannot be stored in a free value: a top-level binding, an owning pointer or a heap node.
The attribute `kind.free` makes a type free, and then neither its parts nor its type arguments may
be scoped. Structures that must hold references for longer use ownership, indices into pools and
arenas, or raw pointers in `unsafe` code. Trees can be built from borrows of an arena:
`std.mem.Arena` hands out `&mut T` tied to the arena's borrow, and
`Expr := enum { lit(i64), add(&Expr, &Expr) }` is an ordinary scoped type. Graphs with cycles use
typed handles into a pool, a library type over nominal structures.

== Pointers

`Own(T)`, the unique owning pointer, is a library structure marked `kind.linear` and `kind.free`
whose pointer field is marked `kind.owns`. It comes from an allocator and returns to one, and
freeing it requires holding the allocator. `.*` on a structure reads through its `owns` field, as
it does on a borrow or pointer, and member lookup continues through it (@sec-syntax-places). The
raw pointer `*T` is copy, free and non-null; `?*T` is nullable and occupies one word. Creating a
raw pointer and reading its address are safe; dereferencing, offsetting, casting and bulk copies
are `unsafe` primitives of `lang.mem`, which `std.ptr` exposes as methods. Volatile and atomic
access are operations, not type qualifiers.

== Layout

The language defines layout; the implementation does not choose it.

- A `struct` is laid out in declaration order with natural alignment and C-style padding. The
  compiler never reorders fields; a lint reports padding and offers a reordering fix.
- A `#[layout.bits(uN)]` structure is bit-exact, least significant bit first, and its fields must
  occupy exactly $N$ bits.
- A `#[layout.c]` structure has the target C ABI's layout, and every field must be of a type C
  reads the same way: full-width integers, `bool`, `f32`, `f64`, raw pointers, C-ABI function
  pointers, arrays of these and other `layout.c` structures.
- `#[layout.tag(uN)]` fixes the tag representation of an enumeration; `#[layout.align(n)]` and
  `#[layout.offset(n)]` raise alignment and place a field.
- An enumeration otherwise uses the smallest integer that holds its tags, except where
  the niche rule below applies.

#rule[niches][An enumeration with one payload-free variant and one payload variant whose type has
unused bit patterns stores the tag in an unused pattern. The unused patterns are the null pointer,
the values of a range integer's storage outside its range, and unused enumeration tags.]

So `?*T` is one pointer, and `?Int(0..200)` occupies one byte, using one of the 56 values of the
byte outside the range. Struct-of-arrays layout is a library type that generates the split layout
by reflection.

== Construction, assignment and passing

Result-location semantics, in which every result is constructed directly in its destination, was
adopted by Zig and has since been judged unsuccessful by its authors @zig-rls: an assignment that
reads its own destination, such as a swap written as a structure literal, observes a partially
written value. Vun restricts the guarantee to where it is sound.

#rule[initialization][Initializing a place in state $U$ constructs the value in that place. This is
guaranteed, not an optimization, and it is what makes fixed types constructible.]

#rule[assignment][Storing into a place in state $I$ evaluates the right-hand side completely and
then moves it in. The temporary may be elided only when the right-hand side provably does not read
the destination.]

Passing by value is a semantic copy or move. An implementation may pass a hidden reference only
when this is unobservable, which is the case for temporaries, moved linear values and locals with
no live borrow. Function addresses are not guaranteed to be unique, so identical lowered functions
may be merged.

== Aliasing facts

The following facts are part of the language definition, so every backend may rely on them.
Exclusive borrows (`&mut`, `&out`, `&take`) alias no other access while they live. A shared
borrow of a cell-free type denotes memory that does not change while the borrow lives. When a
function's result type is free, none of its borrowed parameters escape. Violating these facts
through raw pointers is undefined behavior (@app-ub), and the compile-time evaluator detects it; native code can ask for run-time checks (`--checks`).
