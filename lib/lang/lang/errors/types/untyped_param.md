# types.untyped_param

A parameter of a function has no type.

A function literal's parameters are typed where they are written: nothing else says what a caller
passes, so a run-time parameter written as a name alone, `x`, has no type to be checked against or
computed with. A `$` parameter is generic over its value and needs none.

## Example

```vun
identity := (x) { x }; //~ types.untyped_param
```

## Fix

Write the type of each parameter after its name:

```vun
identity := (x: u8) -> u8 { x };
```
