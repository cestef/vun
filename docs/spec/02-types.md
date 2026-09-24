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
| `&[T]`, `&mut [T]` | slice borrows |
| `*T`, `?*T` | raw pointer, non-null / nullable |
| `(A) -> R` | function type |
| `?T` | `lang.Option(T)` |

**Identity.** Every `struct` and `enum` literal is a new, nominal type, identified by where it is
written and by the compile-time arguments of the evaluation that produced it. Two identical
literals in different declarations are different types, so `NodeId := struct { raw: u32 }` and
`TypeId := struct { raw: u32 }` never mix. A function returning a literal gives one type per set of
arguments (`Pair(u16)` is one type, memoized, spec 03). Aliases are not literals and stay
structural: `u8 := Int(0..=255)` is `Int(0..=255)`.

`bool`, `never` (the empty enum), `Option`, `Result`, `Step` and `Ordering` are ordinary enums
declared in `lang`. Variant constructors are functions: `Result(u8, E).err` has type
`(E) -> Result(u8, E)`.

## 3. Interfaces

An interface is a function from a type to a struct of requirements. `lang.interface` wraps such a
function so that it can be used as a constraint.

```ov
Writer := interface(($Self: type) -> type { struct {
    Error: type = _;                                             // required
    write: (self: &mut Self, bytes: &[u8]) -> Result(usize, Error) = _;
    write_all := (self: &mut Self, bytes: &[u8]) -> Result((), Error) {  // provided default
        mut rest := bytes;
        for rest.len > 0 { rest = rest[self.write(rest)?..]; }
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
operator names is data, the table `operators` of `lang.syntax.grammar`, so the compiler finds them
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
