# int.does_not_fit

A value's range does not lie within the integer type expected where it is used.

Every integer type is a range, and arithmetic is exact: `a + b` has the range computed from its
operands, so it never overflows (spec 04, section 1, rule 1). A value of type `Int(R)` is accepted
where `Int(S)` is expected only when `R` lies within `S` (rule 2); anything else needs an explicit
narrowing, `.to`, `.wrap`, `.sat` or `.try_to` (rule 3). This is the overflow a fixed-width
language would let through at run time.

## Example

```ov
Point := struct { x: i32, y: i32 };

norm1 := (p: Point) -> u32 {
    p.x.abs() + p.y.abs() //~ int.does_not_fit
};
```

## Fix

Declare a type that holds the whole range, here `Int(0..=2^32)`, or narrow explicitly:

```ov
Point := struct { x: i32, y: i32 };

norm1 := (p: Point) -> u64 {
    p.x.abs() + p.y.abs()
};
```
