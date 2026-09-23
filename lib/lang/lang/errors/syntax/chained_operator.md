# syntax.chained_operator

A comparison or range follows another at the same precedence.

Ranges and comparisons do not chain (spec 01, section 6, parse rules): `a < b < c` would compare
a `bool` with `c`, and `a..b..c` has no meaning. Other operators associate to the left.

## Example

```ov
inside := (lo: u8, x: u8, hi: u8) -> bool {
    lo < x < hi //~ syntax.chained_operator
};
```

## Fix

Join the comparisons with `and`, or add parentheses when a chain is really meant:

```ov
inside := (lo: u8, x: u8, hi: u8) -> bool {
    lo < x and x < hi
};
```
