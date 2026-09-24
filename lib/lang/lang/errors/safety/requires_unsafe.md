# safety.requires_unsafe

An operation the compiler cannot check is used outside an `unsafe` block.

Some operations rely on facts the compiler cannot verify, and each use is written inside
`unsafe { }`: dereferencing, offsetting or casting a raw pointer (spec 05, section 5), calling a
function whose type is `unsafe` (spec 07, section 4), any access to a top-level `mut` binding
(spec 01, section 3), and the unsynchronized `lang.mem.Cell` (spec 07, section 2). An `unsafe`
function's body is not implicitly unsafe (spec 01, section 4).

## Example

```vun
peek := (p: *u32) -> u32 {
    p.* //~ safety.requires_unsafe
};
```

## Fix

Wrap the operation in an `unsafe` block, which states that the pointer is valid here:

```vun
peek := (p: *u32) -> u32 {
    unsafe { p.* }
};
```
