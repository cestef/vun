# layout.not_c

A field of a `#[layout.c]` struct has a type C cannot represent.

`#[layout.c]` lays a struct out as the target's C compiler does, so that C code reads and writes
it (spec 05, section 6). C writes every bit pattern of a field, and never promises more than its
declared type, so a field holds only what both sides mean the same way:

- integers of a full width, `u8` to `u64` and `i8` to `i64`: a range such as `Int(0..200)`
  assumes values C may break;
- `bool`, `f32`, `f64`;
- raw pointers `*T`, which are never null, and `?*T` for a pointer C may leave null; a borrow is
  a promise C cannot keep;
- function types marked `#[abi.c]`, and `?` of them;
- arrays of these, and other `#[layout.c]` structs.

A `#[layout.c]` struct has at least one field: C has no empty struct.

## Example

```vun
#[layout.c]
Header := struct {
    tag: Int(0..200), //~ layout.not_c
    len: u32,
};
```

## Fix

Use the full-width type, and narrow the value where it is read:

```vun
#[layout.c]
Header := struct {
    tag: u8,
    len: u32,
};
```
