# types.slice_value

A binding holds a slice by value.

A slice `[T]` is the items of an array of any length (spec 02, section 1): its size is known only
when the program runs, so no binding holds one; it is reached through a borrow, `&[T]` or
`&mut [T]`, which holds its address and length. Indexing an array or a slice with a range,
`xs[1..3]`, names such items; borrow them to keep them.

## Example

```vun
second := () -> u8 {
    xs: [5]u8 = [1, 2, 3, 4, 5];
    ys := xs[1..3]; //~ types.slice_value
    ys[0]
};
```

## Fix

Borrow the items:

```vun
second := () -> u8 {
    xs: [5]u8 = [1, 2, 3, 4, 5];
    ys := &xs[1..3];
    ys[0]
};
```
