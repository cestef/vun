# int.width_cap

An intermediate integer value needs more than 128 bits.

Arithmetic is exact, so each operation widens its result to the range it can reach (spec 04,
section 1, rule 1). Run-time values are capped: an intermediate range that needs more than 128
bits is an error, and the fix is to narrow an intermediate (rule 5). Compile-time-only values have
no cap.

## Example

```vun
cube := (x: u64) -> u64 {
    (x * x * x).to(u64) //~ int.width_cap
};
```

## Fix

Narrow the square before the second multiplication, so no intermediate exceeds 128 bits:

```vun
cube := (x: u64) -> u64 {
    ((x * x).to(u64) * x).to(u64)
};
```
