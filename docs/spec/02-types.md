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

```ov
Pair := ($T: type) -> type { struct { a: T, b: T } };
P := Pair(u16);                            // same arguments, same type (memoized, spec 03)

max := ($T: type, a: T, b: T) -> T { if a > b { a } else { b } };

scratch := () -> u32 {
    $if target.arch == .thumbv7em { cortex_m.cycles() } else { 0 }
};
```

`target` is a compile-time value that the build passes to every package (spec 12). Conditional
compilation is therefore an ordinary `$if`, never a file-name convention or a preprocessor.

## 2. Composite types

| Form | Meaning |
|------|---------|
| `struct { a: A, b: B, d := e; }` | product; fields in declaration order; declarations are its namespace |
| `enum { a, b }` | tags only |
| `enum { none, some(T) }` | sum type with payloads |
| `(A, B)` | tuple |
| `[N]T` | array, a value |
| `[T]` | a slice: the items of an array of any length, reached through a borrow |
| `&[T]`, `&mut [T]` | slice borrows |
| `*T`, `?*T` | raw pointer, non-null / nullable |
| `(A) -> R` | function type |
| `?T` | `lang.Option(T)` |

Arrays and slices are `lang.mem.Array(N, T)` and `lang.mem.Slice(T)`, primitive type constructors
like `lang.Int`, so impls target them: `xs.len()` is the impl `std.seq.Sequence` over the
primitive `lang.mem.len`.

**Identity.** Every `struct` and `enum` literal is a new, nominal type, identified by where it is
written and by the compile-time arguments of the evaluation that produced it. Two identical
literals in different declarations are different types, so `NodeId := struct { raw: u32 }` and
`TypeId := struct { raw: u32 }` never mix. A function returning a literal gives one type per set of
arguments (`Pair(u16)` is one type, memoized, spec 03). Aliases are not literals and stay
structural: `u8 := Int(0..=255)` is `Int(0..=255)`.

`bool`, `never` (the empty enum), `Option`, `Result`, `Step`, `Branch` and `Ordering` are
ordinary enums declared in `lang`. Variant constructors are functions: `Result(u8, E).err` has type
`(E) -> Result(u8, E)`.

A type made by a constructor with an empty enum such as `never` in place of some arguments is
accepted where the same constructor with any arguments there is expected: `Result(never, E)` where
`Result(T, E)` is, since its only values are variants that do not carry the empty type. This is how
`e?` returns what it stops with.

## 3. Interfaces

An interface is a function from a type to a struct of requirements. `lang.interface` wraps such a
function so that it can be used as a constraint.

```ov
Writer := interface(($Self: type) -> type { struct {
    Error: type = _;                                             // required
    write: (self: &mut Self, bytes: &[u8]) -> Result(usize, Error) = _;
    write_all := (self: &mut Self, bytes: &[u8]) -> Result((), Error) {  // provided default
        mut rest := bytes;
        for rest.len() > 0 { rest = rest[self.write(rest)?..]; }
        .ok(())
    };
}});
```

Requirements are declarations whose value is `_` (spec 01, section 3). Declarations with a value
are defaults. There is no separate "associated type" notion: `Error` is a declaration like the
others, and `write` refers to it by name because declarations resolve lazily.

### Conformance

```ov
#[impl(Writer(Uart))]
UartWriter := struct {
    Error := uart.Error;
    write := (self: &mut Uart, bytes: &[u8]) -> Result(usize, uart.Error) { ... };
};
```

- `#[impl(I(T))]` is a `lang` attribute. It checks the struct against the requirements of `I(T)`,
  fills in defaults, and registers the result, the **witness**, for the pair `(I, T)`.
- An impl lives in the package of `T` or of `I` (orphan rule). At most one impl per pair.
- Members of the witness join `T`'s method lookup (spec 01, section 4). A name collision between two
  witnesses, or with `T`'s own members, is an error at the second impl; `UartWriter.write(&mut u, b)`
  is always available as the explicit spelling.
- The witness is an ordinary named struct. `std.meta.witness(Writer, Uart)` returns it.
- `T` may be a type constructor: a primitive such as `lang.Int`, or a function returning a type.
  The impl then covers every type the constructor makes, and its value is a function of the
  constructor's arguments that gives the witness of each. `#[impl(Narrow(Int))]` on
  `IntNarrow := ($r: Range) -> type { struct { ... } }` makes `IntNarrow(r)` the witness for
  `Int(r)`; an impl for `Pair` covers `Pair(u8)`, and a second impl for `Pair(u8)` is a duplicate.
- `T` may also be an interface constructor, a function returning an interface such as
  `lang.Coroutine`: the impl covers every type implementing an interface it makes, and its value
  takes that type first, then the interface's arguments. `#[impl(Iterate(Coroutine))]` on
  `CoroutineIterate := ($Self: type, $Y: type, $R: type) -> type { ... }` makes every coroutine
  frame its own iterator.
- `#[impl(T)]`, with no interface, adds the declarations of its struct to `T`'s method lookup and
  checks nothing else: methods without an abstraction. `T` may be a type or a type constructor,
  as above, so `#[impl(Result)]` on `($T: type, $E: type) -> type { struct { map_err := ... } }`
  gives every result `map_err`. It lives in the package of `T`, and collides like a witness.
- The impls a file sees are those of its package and of the packages it depends on, directly or
  not; the orphan rule places every impl of a pair in one of them.

### Constraints

A compile-time parameter whose annotation is an interface is constrained by it:

```ov
log := ($W: Writer, w: &mut W, msg: &[u8]) -> Result((), W.Error) {
    w.write_all(msg)
};
```

Inside a generic body, a value of type `W` may use only `Writer`'s members. The check runs per
instantiation and is reported against the generic definition.

### Deriving

Because an impl is any expression producing a struct, deriving is calling a function:

```ov
#[impl(Serialize(Point))]
PointSerialize := serde.derive(Point);
```

`serde.derive` is written in spec 03. Nothing about it is special.

### Dynamic dispatch

`std.Dyn(Writer)` reflects over the requirement struct and builds a vtable struct and a fat
pointer. Dynamic dispatch is library code.

## 4. Operators are interfaces

Every operator desugars to an interface in `lang.ops` (appendix B): `a + b` is
`lang.ops.Add.add(a, b)`, `a[i]` is `lang.ops.Index.at(&a, i)`, `a == b` is `lang.ops.Eq.eq(&a, &b)`,
`f(x)` on a non-function value is `lang.ops.Call.call(&f, (x,))`. Which interface and function each
operator names is data, the table `operators` of `lang.syntax.desugar`, so the compiler finds them
by lookup; the result of `a == b` is the result type of `Eq.eq` for the type of `a`, `bool` in
`lang`. `Ord` requires `cmp` and provides `lt`, `le`, `gt` and `ge`; `Eq` provides `ne`. Integer, float and vector
operators are impls whose functions are `_`, provided by the compiler. User types implement the
same interfaces and dispatch statically.

## 5. Attributes

An attribute is any compile-time value, attached with `#[...]` to the next declaration, field,
parameter or type expression:

```ov
#[layout.bits(u32)]
Ctrl := struct { enable: bool, mode: u2, _: u29 };

#[link.symbol("puts"), abi.c]
puts: unsafe (s: *u8) -> i32 = _;       // provided by the linker

#[serde.rename("id")]                   // a user attribute: data for serde.derive
```

- Attributes from `lang` (`layout`, `link`, `abi`, `impl`, `opt`, `panic`, `stack`, `lint`,
  `target`) are acted on by the compiler; each is a documented `lang` declaration.
- Every other attribute is plain data. Compile-time code reads it with `std.meta.attrs(x)`.
- One mechanism serves layout, linking, ABIs, lints, derives, tests and serialization options.
