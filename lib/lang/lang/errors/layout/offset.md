# layout.offset

A field's `layout.offset(N)` places it where it cannot be.

`#[layout.offset(N)]` places a field `N` bytes into its struct (spec 05, section 6). The
compiler never reorders fields, so `N` is at or past the end of the field before it, and it is a
multiple of the field type's alignment.

## Example

```vun
Regs := struct {
    #[layout.offset(4)]
    status: u32,
    #[layout.offset(0)]
    control: u32, //~ layout.offset
};
```

## Fix

Declare the fields in the order of their offsets:

```vun
Regs := struct {
    #[layout.offset(0)]
    control: u32,
    #[layout.offset(4)]
    status: u32,
};
```
