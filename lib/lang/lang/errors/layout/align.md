# layout.align

The alignment of `layout.align(N)` is not a power of two.

`#[layout.align(N)]` raises a type's alignment to `N` bytes (spec 05, section 6). Every alignment
is a power of two, so `N` must be one.

## Example

```vun
#[layout.align(3)]
Block := struct { data: [12]u8 }; //~ layout.align
```

## Fix

Align to a power of two:

```vun
#[layout.align(4)]
Block := struct { data: [12]u8 };
```
