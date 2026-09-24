# borrow.in_free_value

A borrow is stored in a value that is not scoped.

A scoped value may live in locals, parameters and fields of other scoped types, but a borrow cannot
be stored in a free value: a top-level binding, an `Own`, a heap node (spec 05, section 4). A type
is made free by `#[kind.free]`, and then neither its parts nor its type arguments may be scoped;
the error is reported where such a value is built. A free value can outlive every scope, so the
borrow would dangle. Long-lived structures use ownership,
indices into pools and arenas, or raw pointers in `unsafe` code.

## Example

```vun
Parser := struct { input: &[u8] };
Boxed := #[kind.free] struct { parser: Parser };

boxed := (p: Parser) -> Boxed {
    .{ .parser = p } //~ borrow.in_free_value
};
```

## Fix

Keep the scoped value in a scoped place, such as a local or the caller's result, tied to `input`:

```vun
Parser := struct { input: &[u8] };

parser := (input: &[u8]) -> Parser {
    .{ .input = input }
};
```
