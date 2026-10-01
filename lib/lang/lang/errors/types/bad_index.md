# types.bad_index

An array or a slice is indexed by a value that is neither an integer nor bounds.

`xs[i]` takes an integer `i` and gives an item; `xs[a..b]`, `xs[a..=b]` and `xs[a..]` take bounds
and give the items they cover (appendix B). Every bound is written: `..b` is not a range, since a
prefix `..` marks a pack (spec 01, section 2), so `xs[..b]` indexes by a type.

## Example

```vun
prefix := (xs: &[u8], n: usize) -> &[u8] {
    xs[..n] //~ types.bad_index
};
```

## Fix

Write the start of the range:

```vun
prefix := (xs: &[u8], n: usize) -> &[u8] {
    xs[0..n]
};
```
