# `std.lints.layout.pub_doc`

A declaration that any package sees says what it is in a doc comment: a type, a function, a
constant, a field or a variant of a public type. The lint is allowed unless a build asks for it,
as a library publishing an interface does.

```vun
Buffer := struct { len: usize };    // reported: the public type `Buffer` has no doc comment

/// Bytes waiting to be written.
Buffer := struct {
    /// How many are waiting.
    len: usize,
};
```

A name starting with `_` is private and never reported.
