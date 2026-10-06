#import "../template.typ": *

= Types and interfaces <sec-types>

== Types are values

A type in Vun is a value of type `type`. A function that takes types and returns a type is a type
constructor, and a function with type parameters is a generic function; neither needs dedicated
syntax. A compile-time parameter is marked with `$`, and each distinct argument yields a distinct
instantiation.

```vun
Pair := ($T: type) -> type { struct { a: T, b: T } };
P := Pair(u16);                  // the same arguments give the same type

max := ($T: type, a: T, b: T) -> T { if a > b { a } else { b } };
```

A compile-time parameter named in the type of a later parameter may be omitted from a call, in
which case it is inferred by matching the types of the arguments against the declared types:
`max(3, x)` with `x: u8` instantiates `T` as `u8`, since the literal `3` has the singleton type
`Int(3..=3)` and the join of both is `u8`. Either every such parameter is written or none is. A
parameter that no argument determines is an error.

Conditional compilation is an ordinary compile-time branch on `target`, a value the build passes to
every package; there is no preprocessor and no file-name convention.

```vun
scratch := () -> u32 {
    $if target.cpu == .cortex_m4f { cortex_m.cycles() } else { 0 }
};
```

== Composite types and identity

The composite types are structures (`struct { a: A, b: B }`), enumerations with optional payloads
(`enum { none, some(T) }`), tuples, arrays `[N]T`, slices `[T]` reached through a borrow, raw
pointers `*T`, borrows, function types and optionals `?T`. A structure body and, after a `;`, an
enumeration body may hold declarations, which form the type's namespace. Arrays and slices are the
primitive constructors `lang.mem.Array(N, T)` and `lang.mem.Slice(T)`, so implementations can
target them like any other constructor. `bool`, `never` (the empty enumeration), `Option`,
`Result` and `Ordering` are ordinary enumerations declared in `lang`; `true` and `false` are names,
not keywords.

Identity follows one rule. Every `struct` and `enum` literal is a new _nominal_ type, identified by
the literal's origin and by the compile-time arguments of the evaluation that produced it. Two
identical literals in different declarations are distinct types, so that handle types such as
`NodeId := struct { raw: u32 }` and `TypeId := struct { raw: u32 }` never mix. Because
compile-time calls are memoized, `Pair(u16)` is the same type wherever it is written. _Aliases_ are
not literals and remain structural: `u8 := Int(0..=255)` is exactly `Int(0..=255)`. An anonymous
structure `.{ .a = x, .b = y }` written where no type is expected is a _record_, a structural type
identified by its sorted field names and types.

A type built with an empty type in place of some arguments is accepted wherever the same
constructor is expected with any arguments there: `Result(never, E)` is accepted as a
`Result(T, E)`, since its only values are variants that do not carry the empty type. This is how
the postfix `?` returns the value it stops with without converting it.

== Interfaces

An interface is a function from a type to a structure of requirements. Requirements are
declarations whose value is `_`; declarations with a value are defaults. There is no separate
notion of an associated type: `Error` below is a declaration like the others, and `write` refers
to it by name because declarations resolve lazily.

```vun
Writer := interface(($Self: type) -> type { struct {
    Error: type = _;
    closed: Error = _;
    write: (self: &mut Self, bytes: &[u8]) -> Result(usize, Error) = _;
    write_all := (self: &mut Self, bytes: &[u8]) -> Result((), Error) {
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

Conformance is the `lang` attribute `#[impl(I(T))]` on any expression that produces a structure.
The attribute checks the structure against the requirements of `I(T)`, fills in defaults, and
registers the result, called the _witness_, for the pair `(I, T)`; a witness made for a use, by a
type constructor or for the values of an argument written `_`, is checked where it is used. An
implementation lives in the package of `T` or of `I`, and there is at most one per pair. Because
an implementation is any structure-valued expression, deriving an implementation is calling a
function:

```vun
#[impl(Serialize(Point))]
point_serialize := serde.derive(Point);
```

Declarations `impl I for T { ... }` and `impl T { ... }` are sugar for the attribute. An interface
marked `#[derive(f)]` gives a witness to every structure or enumeration whose parts all have it, so
`==` and hashing need no written implementation. Three generalizations use the same attribute
without adding a mechanism.

- *Constructors as targets.* When `T` is a type constructor, such as `lang.Int` or a user function
  returning a type, the implementation covers every type the constructor makes, and its value is
  a function of the constructor's arguments. The methods of built-in types are provided this way,
  in the library: the narrowing methods of every integer type are one implementation of
  `std.int.Narrow` for the constructor `Int`.
- *Interface constructors as targets.* When the target is a function returning an interface, the
  implementation covers every type implementing an interface it makes. A single implementation of
  `Iterate` over the interface constructor `Coroutine` makes every coroutine frame its own
  iterator (@sec-suspension).
- *Methods without an interface.* `#[impl(T)]` without an interface adds the declarations of its
  structure to the method lookup of `T` and checks nothing else. No pair is registered, so the
  orphan rule does not apply; a name declared by two implementations visible to a file, or by an
  implementation and the type itself, is an error where it is looked up.

A compile-time parameter annotated with an interface is constrained by it. Inside the generic
body, a value of that type may use only the interface's members; the check runs per instantiation
and is reported against the generic definition.

```vun
log := ($W: Writer, w: &mut W, msg: &[u8]) -> Result((), W.Error) {
    w.write_all(msg)
};
```

Function types denote plain function values, which are static and capture nothing. Code that
takes a callback is generic over the interface `lang.ops.Call`, which both function values and
closures implement, so either can be passed with static dispatch. Dynamic dispatch is a library
type, `std.Dyn(I)`, which reflects over the requirement structure of `I` to build a table of
function pointers and a pointer to the target, and gives each requirement as a method that calls
through the table.

== Operators and attributes

Every operator desugars to an interface of `lang.ops`: `a + b` is `lang.ops.Add.add(a, b)`,
`a == b` is `lang.ops.Eq.eq(&a, &b)`, `a[i]` is `lang.ops.Index.at(&a, i)`. The table that maps
tokens to interfaces is the one shown in @sec-principles. The comparison operators are
`PartialOrd`, a partial order: floats implement it with IEEE comparisons, a NaN unordered with
every value. `Ord` is a total order, requiring only `cmp`, and `lang.ops.by_cmp` derives a type's
`PartialOrd` from it, so that operators and `cmp` never disagree. `Eq` provides `ne`. Integer, floating-point and vector operators are
implementations whose functions are `_`, supplied by the compiler; user types implement the same
interfaces and dispatch statically.

An attribute is any compile-time value attached with `#[...]` to the next declaration, field,
parameter, type expression or expression. Attributes from `lang` namespaces (`layout`, `link`,
`abi`, `kind`, `opt`, `panic`, `stack`, `target`, `comptime`, `lint`, `check`, `safety`) and the
attributes `impl`, `derive` and `visible` are acted on by the compiler,
and each is a documented `lang` declaration whose type, `lang.Attribute(on)`, lists the places it
applies to (declarations, functions, types, fields, parameters); one written elsewhere is an error
rather than ignored, and a library attribute may declare its places the same way. Every other
attribute is plain data that compile-time code reads through reflection. One mechanism thus serves layout, linking, calling
conventions, lints, derivation, tests and serialization options.

```vun
#[layout.bits(u32)]
Ctrl := struct { enable: bool, mode: u2, _: u29 };

#[link.symbol("puts"), abi.c]
puts: unsafe (s: *u8) -> i32 = _;       // provided by the linker
```
