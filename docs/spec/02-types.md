# 02. Types as compile-time values

## 1. `$`: known at compile time

A type is a value of type `type`. Values that must be known during compilation are marked with `$`,
and `$` has one meaning everywhere: **evaluate this one stage earlier**. In ordinary code that means
"at compile time"; inside a quote it means "at generation time" (spec 03).

| Form | Meaning |
|------|---------|
| `($T: type, x: T)` | `T` must be known at compile time; each distinct `T` is a distinct instantiation |
| `$e` | evaluate `e` at compile time; an error if it cannot be |
| `$for x in xs { ... }` | unroll over a compile-time sequence |
| `$if c { ... } else { ... }` | choose a branch at compile time; the other branch is parsed and name-resolved, not type-checked |
| `${ ... }` | a block run entirely at compile time |

A value known at compile time may be used at run time without ceremony: it becomes a constant.

A `$` parameter named in the type of a later parameter may be left out of a call: either every
such parameter or none. Its value is inferred by matching the types of the arguments against the
declared types, and a type found twice is the join of both, as for the branches of an `if`;
`max(3, x)` for `x: u8` has `T = u8`. A parameter no argument determines is an error
(`types.cannot_infer`), and one no later type names, such as the `T` of `a.create(T)`, is always
written. A variant with a payload is a function value: `E.fs` for `fs(u8)` has type
`(u8) -> E`.

```vun
Pair := ($T: type) -> type { struct { a: T, b: T } };
P := Pair(u16);                            // same arguments, same type (memoized, spec 03)

max := ($T: type, a: T, b: T) -> T { if a > b { a } else { b } };

scratch := () -> u32 {
    $if target.current.cpu == .cortex_m4f { cortex_m.cycles() } else { 0 }
};
```

`target.current` is a compile-time value, the target the build passes to every package (spec 12). Conditional
compilation is therefore an ordinary `$if`, never a file-name convention or a preprocessor.

## 2. Composite types

| Form | Meaning |
|------|---------|
| `struct { a: A, b: B, d := e; }` | product; fields in declaration order; declarations are its namespace |
| `enum { a, b }` | tags only |
| `enum { none, some(T) }` | sum type with payloads |
| `enum { a, b; d := e; }` | variants, then after `;` declarations: its namespace, as a struct's |
| `.{ .a = x, .b = y }` where no type is expected | a record: structural, its fields sorted by name, so records with the same fields are one type; `.{ x, y }` is a tuple |
| `.name` where no type is expected | a `lang.meta.Name` value |
| `(A, B)` | tuple |
| `[N]T` | array, a value |
| `[T]` | a slice: the items of an array of any length, reached through a borrow |
| `&[T]`, `&mut [T]` | slice borrows |
| `*T`, `?*T` | raw pointer, non-null / nullable |
| `(A) -> R` | function type |
| `?T` | `lang.Option(T)` |

Arrays and slices are `lang.mem.Array(N, T)` and `lang.mem.Slice(T)`, primitive type constructors
like `lang.Int`, so impls target them: `xs.len()` is the impl `std.seq.Sequence` over the
primitive `lang.mem.len`. A slice's size is known only at run time, so a binding holding one by
value, as `ys := xs[1..3]`, is `types.slice_value`, whose fix borrows it (`&xs[1..3]`).

**Identity.** Every `struct` and `enum` literal is a new, nominal type, identified by where it is
written and by the compile-time arguments of the evaluation that produced it. Two identical
literals in different declarations are different types, so `NodeId := struct { raw: u32 }` and
`TypeId := struct { raw: u32 }` never mix. A function returning a literal gives one type per set of
arguments (`Pair(u16)` is one type, memoized, spec 03). The same holds for a literal in the body
of a function with `$` parameters and run-time ones: it is identified by the instantiation that
evaluates it, so `B := struct { v: [n]u8 }` in `($n: u8, x: u8)` is one type per `n`, and a
function literal or closure written there sees `n` as the body does (decision 362). Aliases are not literals and stay
structural: `u8 := Int(0..=255)` is `Int(0..=255)`.

`bool`, `never` (the empty enum), `Option`, `Result`, `Step`, `Branch` and `Ordering` are
ordinary enums declared in `lang`. Variant constructors are functions: `Result(u8, E).err` has type
`(E) -> Result(u8, E)`. `true` and `false` are `lang` names for `bool.true` and `bool.false`, in
the prelude; they are not keywords.

A type made by a constructor with an empty enum such as `never` in place of some arguments is
accepted where the same constructor with any arguments there is expected: `Result(never, E)` where
`Result(T, E)` is, since its only values are variants that do not carry the empty type. This is how
`e?` returns what it stops with.

## 3. Interfaces

An interface is a function from a type to a struct of requirements. `lang.interface` wraps such a
function so that it can be used as a constraint.

```vun
Writer := interface(($Self: type) -> type { struct {
    Error: type = _;                                             // required
    closed: Error = _;
    write: (self: &mut Self, bytes: &[u8]) -> Result(usize, Error) = _;
    write_all := (self: &mut Self, bytes: &[u8]) -> Result((), Error) {  // provided default
        mut rest := bytes;
        for rest.len() > 0 {
            n := self.write(rest)?;
            if n == 0 { return .err(closed); }
            rest = rest[n..];
        }
        .ok(())
    };
}});
```

Requirements are declarations whose value is `_` (spec 01, section 3). Declarations with a value
are defaults. There is no separate "associated type" notion: `Error` is a declaration like the
others, and `write` and `closed` refer to it by name because declarations resolve lazily. `closed`
is the error `write_all` reports when a write takes none of its bytes, so that it never loops on a
writer that takes no more.

### Conformance

```vun
#[impl(Writer(Uart))]
uart_writer := struct {
    Error := uart.Error;
    closed: uart.Error = .stopped;
    write := (self: &mut Uart, bytes: &[u8]) -> Result(usize, uart.Error) { ... };
};
```

- `#[impl(I(T))]` is a `lang` attribute. It checks the struct against the requirements of `I(T)`
  (each present, with a type the requirement's declared type accepts), fills in defaults, and
  registers the result, the **witness**, for the pair `(I, T)`. A witness made for a use, by a type
  constructor or for the values of `_` (below), is checked where it is used: a method found through
  it reports each requirement it does not meet at the use, the impl labeled.
- An impl lives in the package of `T` or of `I` (orphan rule). At most one impl per pair.
- Members of the witness join `T`'s method lookup (spec 01, section 4). A name declared by two
  impls a file sees, or by an impl and `T` itself, is an error where it is looked up
  (`impl.collision`), naming each declaration; `uart_writer.write(&mut u, b)` is always available
  as the explicit spelling.
- The witness is an ordinary named struct. `std.meta.witness(Writer, Uart)` returns it.
- `T` may be a type constructor: a primitive such as `lang.Int`, or a function returning a type.
  The impl then covers every type the constructor makes, and its value is a function of the
  constructor's arguments that gives the witness of each. `#[impl(Narrow(Int))]` on
  `int_narrow := ($r: Range) -> type { struct { ... } }` makes `int_narrow(r)` the witness for
  `Int(r)`; an impl for `Pair` covers `Pair(u8)`, and a second impl for `Pair(u8)` is a duplicate.
- `T` may also be an interface constructor, a function returning an interface such as
  `lang.Coroutine`: the impl covers every type implementing an interface it makes, and its value
  takes that type first, then the interface's arguments. `#[impl(Iterate(Coroutine))]` on
  `coroutine_iterate := ($Self: type, $Y: type, $R: type) -> type { ... }` makes every coroutine
  frame its own iterator.
- An argument of the interface may be `_`: the impl covers every interface the constructor makes
  with any value there, and its value takes those values, one per `_` in order, after what the
  target gives. `#[impl(lang.ops.Call(_, ())(Instruction))]` on
  `instruction_call := ($Params: type) -> type { ... }` makes an instruction callable with any
  arguments: a call finds the witness for the tuple of its arguments' types (spec 07, section 6).
  `_` means any here as it does in a pattern; anywhere else as a value it is an error.
- `#[impl(T)]`, with no interface, adds the declarations of its struct to `T`'s method lookup and
  checks nothing else: methods without an abstraction. `T` may be a type or a type constructor,
  as above, so `#[impl(Result)]` on `($T: type, $E: type) -> type { struct { map_err := ... } }`
  gives every result `map_err`. It registers no pair, so the orphan rule does not apply: any
  package may add methods to any type, and the collision rule above keeps lookup unambiguous.
- The impls a file sees are those of its package and of the packages it depends on, directly or
  not; the orphan rule places every impl of a pair in one of them. A type's impls are also looked
  up in the package declaring it, so a generic body instantiated with a type from a package it
  does not depend on still finds the type's witness.

### Constraints

A compile-time parameter whose annotation is an interface is constrained by it:

```vun
log := ($W: Writer, w: &mut W, msg: &[u8]) -> Result((), W.Error) {
    w.write_all(msg)
};
```

Inside a generic body, a value of type `W` may use only `Writer`'s members, and so may `W`
itself: `W.Error` names the requirement `Error`. Its value, as that of any requirement, and the
value of a `$` parameter no constraint makes abstract, as `T` of `$T: type`, are known only in
an instantiation: the generic body is typed as far as they are not needed, and the rest is
checked per instantiation (decision 360). The check runs per instantiation and is reported
against the generic definition. This is the rule, not a stage towards typing generic bodies
abstractly (decision 383): a `$` value is a compile-time value like any other, which a body may
compute with (`$if`, `$for`, `lang.meta`), so no opaque stand-in could type what it decides.

A function type `(T) -> U` is a plain function value: static and capture-free. Code taking a
callback is generic over `lang.ops.Call` (the table `calls`), which function values and closures
both implement, so it takes either with static dispatch:
`map := (o: ?T, $U: type, $F: lang.ops.Call((T), U), f: F) -> ?U`. When `$F` is inferred from
`f`, the arguments of its constraint are inferred from the type found: `U` is the result of the
function or closure passed, and inside a generic body, where the type found is a `$` parameter,
the arguments of the constraint that parameter is declared with. A function or closure answers
that interface's requirement itself: `f.call((a, b))` is `f(a, b)`, so code holding the arguments
as a tuple calls with them (decision 361).

A function with `$` parameters has no function type: its signature depends on the `$` arguments
of a call. Named as a value, it is of its own type, whose only value it is, and that type exists
at compile time only; calling it instantiates it (decision 359). It is accepted where a function
type is expected, as an impl's `lt := lang.int.less` meets `lt: (a: &Self, b: &Self) -> bool`:
the call instantiating it is checked, as a generic body is.

### Deriving

Because an impl is any expression producing a struct, deriving is calling a function:

```vun
#[impl(Serialize(Point))]
point_serialize := serde.derive(Point);
```

`serde.derive` is written in spec 03. Nothing about it is special.

### Dynamic dispatch

`std.Dyn(Writer)` reflects over the requirement struct and builds a vtable struct and a fat
pointer. Dynamic dispatch is library code.

## 4. Operators are interfaces

Every operator desugars to an interface in `lang.ops` (appendix B): `a + b` is
`lang.ops.Add.add(a, b)`, `a[i]` is `lang.ops.Index.at(&a, i)`, `a == b` is `lang.ops.Eq.eq(&a, &b)`,
`f(x)` on a non-function value is `lang.ops.Call.call(&f, (x,))`, and on a borrow of a function or
closure calls what it borrows. Which interface and function each
operator names is data, the table `operators` of `lang.syntax.desugar`, so the compiler finds them
by lookup; the result of `a == b` is the result type of `Eq.eq` for the type of `a`, `bool` in
`lang`. The comparison operators are `PartialOrd`, which requires `lt` and `le` and provides `gt`
and `ge`: a partial order, in which two values may be unordered, every comparison of them false.
`Ord` is a total order and requires only `cmp`; a totally ordered type's `PartialOrd` is
`lang.ops.by_cmp(T)`, so its operators and `cmp` never disagree. Floats implement only
`PartialOrd`, with IEEE comparisons (a NaN is unordered); IEEE `totalOrder` is the function
`lang.float.total_order`. `Eq` provides `ne`. The
arithmetic, bit and concatenation interfaces take the right operand's type per call
(`add: ($R: type, a: Self, b: R) -> Output(R)`), since an integer result's range depends on both
operands. Prefix `-` and `~` are `lang.ops.Neg` and `Not` (the table `prefix_operators`); `!`
negates a condition. There is no other operator rule: every operand type, integers included, goes
through its impl, and the operands are passed as the function's parameters ask, each reached
through its borrows as `.` reaches a value. Integer, float and vector operators are impls whose
functions are `_`, provided by the compiler: `lang.int.ops` gives `add := lang.int.add`, whose
signature carries spec 04's rules through range primitives (`Int(lang.int.sum(a, b))`). A
compound assignment and a literal or range pattern are operators too. User types implement the
same interfaces and dispatch statically.

**Derived impls.** An interface marked `#[derive(f)]` gives a struct or an enum with no impl of it
the witness `f(T)`, when every part of it, each field and the payload of each variant, has the
interface too; `f` builds the witness by reflection. `Eq` and `std.hash.Hash` are marked, so
`a == b` compares structs and enums by their parts, and they are hashed by them, without an impl
(`lang.ops.derive_eq`, `std.hash.derive_hash`). An impl written for the type takes the place of the
derived one, and a type with a part lacking the interface has none: `types.no_impl`.

## 5. Attributes

An attribute is any compile-time value, attached with `#[...]` to the next declaration, field,
parameter or type expression:

```vun
#[layout.bits(u32)]
Ctrl := struct { enable: bool, mode: u2, _: u29 };

#[link.symbol("puts"), abi.c]
puts: unsafe (s: *u8) -> i32 = _;       // provided by the linker

#[serde.rename("id")]                   // a user attribute: data for serde.derive
```

- Attributes from `lang` (`layout`, `link`, `abi`, `impl`, `opt`, `panic`, `stack`, `lint`,
  `target`) are acted on by the compiler; each is a documented `lang` declaration.
- An attribute's type says where it applies: `lang.Attribute(on)`, for `on` a list of
  `lang.Place` (`decls`, `functions`, `types`, `fields`, `params`), is the type of an attribute
  written only there, as `c: Attribute(&[Place.types]) = _;` of `lang.layout`. A declaration is
  at `decls`, and also at `functions` or `types` when its value is a function or a type literal
  (or it provides a function from outside); an attributed function or type expression is at
  `functions` or `types`. One written elsewhere is `attr.misplaced`. Every `lang` attribute
  declares its places; a library attribute may, by the same type.
- Every other attribute is plain data, written anywhere. Compile-time code reads it with
  `std.meta.attrs(x)`.
- The compiler reads an item's attributes one way (`Attributes` of a function, type or
  declaration: where they are written and the environment evaluating them), and markers
  (spec 07, section 4) only as markers.
- One mechanism serves layout, linking, ABIs, lints, derives, tests and serialization options.
