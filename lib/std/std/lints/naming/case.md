# `std.lints.naming.case`

A name's case follows what it binds (spec 10, section 5): a type, an interface or a function
giving a type is named in `PascalCase`; every other binding, values, functions, fields and
variants, in `snake_case`. A leading `_`, which marks a private name, is not part of the case.

```vun
ring_buffer := struct { len: usize };    // reported: the type `ring_buffer` should be named in `PascalCase`
RingBuffer := struct { len: usize };
```

A binding is a type by how it is written: a `struct`, an `enum`, an `interface`, a function type
or a function literal giving a type, or one annotated `type` or `lang.Interface`. A name or a call
that gives a type is a value, so an alias is annotated to bind one:

```vun
Index: type = Int(0..9);
Error: type = ();
```

An alias of a type constructor, a function giving a type, is annotated with the function type
that says so, since it is no `type` itself:

```vun
Failure: ($E: type) -> type = std.ffi.cstr.Failure;
```

A `struct` holding no fields, only declarations, is a namespace and is named as a value
(`snake_case`); one that holds a field is a type.

The built-in type names keep their conventional lowercase (`u8`, `bool`, `type`, ...): they are
listed in `std.lints.naming.lowercase_types`. Allow the lint where a name mirrors one written
elsewhere, and say why:

```vun
#[lint.allow(std.lints.naming.case, "mirrors the vendor register names")]
HAL_UART_Init := 0;
```
