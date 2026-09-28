# types.pack_position

A pack is not a function's last parameter.

A pack `..args: P` gathers the arguments of a call after the others into a tuple of type `P`
(spec 01, section 4), so it can only be the last parameter: the arguments before it each have
their own.

## Example

```vun
sum := (..xs: .{ u8, u8 }, last: u8) -> u8 { last }; //~ types.pack_position
```

## Fix

Write the pack last:

```vun
sum := (last: u8, ..xs: .{ u8, u8 }) -> u8 { last };
```
