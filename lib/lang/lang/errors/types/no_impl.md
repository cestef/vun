# types.no_impl

An operation needs an impl of an interface that the type does not have.

Every operator desugars to an interface in `lang.ops` (spec 02, section 4), so `a & b` exists only
for types with an impl of the bit-and interface. The integer impls follow the rules of spec 04,
section 1: bit operations exist on `uN` types only (rule 8).

## Example

```vun
low_byte := (x: i32) -> i32 {
    x & 0xff //~ types.no_impl
};
```

## Fix

Use an unsigned type, which has the bit operations:

```vun
low_byte := (x: u32) -> u32 {
    x & 0xff
};
```
