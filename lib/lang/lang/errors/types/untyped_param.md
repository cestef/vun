# types.untyped_param

A parameter of a function has no type.

A function literal's parameters are typed where they are written: nothing else says what a caller
passes, so a run-time parameter written as a name alone, `x`, has no type to be checked against or
computed with. A `$` parameter is generic over its value and needs none, and a literal that is
the value of a declaration annotated with a function type of as many parameters takes its
parameters' types, and its result, from that type.

## Example

```vun
identity := (x) { x }; //~ types.untyped_param
```

## Fix

Write the type of each parameter after its name:

```vun
identity := (x: u8) -> u8 { x };
```

Or annotate the declaration the literal is the value of with a function type:

```vun
identity: (x: u8) -> u8 = (x) { x };
```
