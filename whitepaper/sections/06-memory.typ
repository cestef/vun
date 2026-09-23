#import "../template.typ": *

= Places, kinds and borrows <sec-memory>

== Kinds

Every type has three structural properties, computed from its members; a declaration may make a
type more restrictive but never less.

- *Duplication*: a type is _copy_ if all its members are copy and it is not declared linear;
  otherwise it is _linear_, and each of its values must be consumed exactly once.
- *Region*: a type is _scoped_ if it contains a borrow, and _free_ otherwise.
- *Address*: a type is _fixed_ if its values must not change address after construction, and
  _movable_ otherwise.

A fourth fact is derived and cannot be declared: a type is _cell-free_ if it contains no
unsynchronized interior-mutable cell. There are no destructor, clone, thread-safety, pinning or
sizedness traits; their roles are taken by these properties and by capabilities
(@sec-capabilities). Linear types in the tradition of Austral @austral make resource release an
obligation checked by the compiler instead of an implicit call.

== Place states

Every place, meaning a local, a field, an element or the target of a write-only borrow, is in one of
two states, $U$ (uninitialized) or $I$ (initialized). The borrow forms of the language are named by
the transition they perform on the place they borrow.

#figure(
  table(
    columns: (auto, auto, auto, 1fr),
    table.hline(),
    [Form], [Before], [After], [Access],
    table.hline(stroke: 0.5pt),
    [`&x`], [$I$], [$I$], [shared read; any number may coexist],
    [`&mut x`], [$I$], [$I$], [exclusive read and write],
    [`&out x`], [$U$], [$I$], [exclusive write; the callee must initialize],
    [`&take x`], [$I$], [$U$], [exclusive; the callee must consume in place],
    [`&temp` parameter], [$I$], [$I$], [lent for the call; the result is not tied to it],
    [move of a linear value], [$I$], [$U$], [ownership leaves the place],
    [copy], [$I$], [$I$], [a duplicate is made],
    table.hline(),
  ),
  caption: [Borrow forms and moves as transitions on place states.],
) <tab-places>

The checker is a forward dataflow analysis over the control-flow graph of each function, computing a
state for every place at every program point. Its rules are few.

#rule[reads][Reading a place requires state $I$. An uninitialized declaration is legal and costs
nothing; reading it before a write is a compile-time error. No `undefined` value exists.]

#rule[exits][At every exit of a scope, every place of a linear type declared in that scope is in
state $U$.]

#rule[joins][A place whose state differs between the incoming edges of a join point is unusable
after the join, and violates the exit rule if linear.]

The exit rule is what replaces destructors. Forgetting to close a file, free an allocation, or end a
transfer is a compile-time error, and cleanup is always a call visible in the source. To keep that
ergonomic, `defer s` schedules `s` at every exit of the enclosing block, with one refinement that
follows from place states: if `s` consumes a linear place, it runs exactly on the exits where that
place is in state $I$, and is omitted on the exits where the value was moved away. Because states are
computed statically, the choice needs no run-time flag, and the separate construct for
"cleanup on error only" found in other languages is unnecessary.

```ov
load := (fs: &mut Fs, a: &mut Alloc, path: &[u8]) -> Result(Own(Config), Error) {
    f := fs.open(path).map_err(Error.fs)?;
    defer fs.close(f);
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
argument borrowed, even those it does not derive from. In exchange, borrows never escape except
through returns tied to arguments, so exclusivity is checked per function, with non-lexical
liveness, and no interprocedural analysis is required. Borrows of immutable file-level bindings form
a second region, `&static`, whose borrows are free and copy and are accepted where scoped borrows are
expected; this is the language's only subtyping rule. Structures that must hold references for longer
use ownership, indices into pools, or raw pointers in `unsafe` code.

== Construction, assignment and passing

Result-location semantics, in which every result is constructed directly in its destination, was
adopted by Zig and has since been judged a failed experiment @zig-rls: an assignment that reads its
own destination, such as a swap written as a struct literal, observes a half-written value. Overt
restricts the guarantee to where it is sound.

#rule[initialization][Initializing a place in state $U$ constructs the value in that place. This is
guaranteed, never an optimization, and it is what makes fixed types constructible.]

#rule[assignment][Storing into a place in state $I$ evaluates the right-hand side completely and then
moves it in. The temporary may be elided only when the right-hand side provably does not read the
destination.]

Passing by value is a semantic copy or move; an implementation may pass a hidden reference only when
that is unobservable, which is the case for temporaries, moved linear values, and locals with no live
borrow.

== Aliasing facts

Because the facts below are part of the language definition, every backend may rely on them:
exclusive borrows (`&mut`, `&out`, `&take`) do not alias any other access; a shared borrow of a
cell-free type denotes memory that does not change while the borrow lives; and when a function's
result is free, none of its borrowed parameters escape. Violating these through raw pointers is
undefined behavior, and the reference interpreter detects it.
