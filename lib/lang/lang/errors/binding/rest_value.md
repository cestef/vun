# binding.rest_value

`..` is written where a value is expected.

A lone `..` skips the items a pattern leaves (spec 01, section 6): `(first, ..)` matches a tuple by
its first item. It has no value of its own, so `xs[..]` does not take every item of `xs`, and
nothing else may take `..` as an operand either.

## Example

```vun
all := (xs: &[u8]) -> &[u8] {
    xs[..] //~ binding.rest_value
};
```

## Fix

Write the bounds of the range:

```vun
all := (xs: &[u8]) -> &[u8] {
    xs[0..xs.len()]
};
```
