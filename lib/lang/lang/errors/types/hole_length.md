# types.hole_length

A parameter's type leaves an array length as `_`.

`[_]u8` is a type whose length is the one the value it is initialized with has. Where nothing
is initialized, a parameter, a field, a result, a binding without a value, no length is given: each
call or value has a length of its own.

## Example

```vun
fill := (into: &mut [_]u8) { into[0] = 1; }; //~ types.hole_length
```

## Fix

Take a slice, which an array of any length borrows as:

```vun
fill := (into: &mut [u8]) { into[0] = 1; };
```

Or write the length, as a `$` parameter when callers differ:

```vun
fill := ($N: usize, into: &mut [N]u8) { into[0] = 1; };
```
