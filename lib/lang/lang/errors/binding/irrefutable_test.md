# binding.irrefutable_test

An `if` or `for` head tests a pattern that always matches.

`if p = e` and `for p = e` test whether `e` matches `p`. With an irrefutable `p` the test is
always true, which is an error: bind with `:=` instead (spec 01, section 6).

## Example

```ov
first := (pair: (u8, u8)) -> u8 {
    if (a, b) = pair { a } else { 0 } //~ binding.irrefutable_test
};
```

## Fix

Bind the pattern, since any irrefutable pattern binds:

```ov
first := (pair: (u8, u8)) -> u8 {
    (a, _) := pair;
    a
};
```
