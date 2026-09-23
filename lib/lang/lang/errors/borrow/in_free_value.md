# borrow.in_free_value

A borrow is stored in a value that is not scoped.

A scoped value may live in locals, parameters and fields of other scoped types, but a borrow cannot
be stored in a free value: a top-level binding, an `Own`, a heap node (spec 05, section 4). A free
value can outlive every scope, so the borrow would dangle. Long-lived structures use ownership,
indices into pools and arenas, or raw pointers in `unsafe` code.

## Example

```ov
Parser := struct { input: &[u8] };

boxed := (
    $A: std.mem.alloc.Allocator,
    a: &mut A,
    input: &[u8],
) -> Result(std.mem.Own(Parser), A.Error) {
    a.create(Parser{ .input = input }) //~ borrow.in_free_value
};
```

## Fix

Keep the scoped value in a scoped place, such as a local or the caller's result, tied to `input`:

```ov
Parser := struct { input: &[u8] };

parser := (input: &[u8]) -> Parser {
    .{ .input = input }
};
```
