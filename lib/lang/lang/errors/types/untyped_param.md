# types.untyped_param

A parameter of a function has no type.

A function literal's parameters are typed where they are written: nothing else says what a caller
passes, so a run-time parameter written as a name alone, `x`, has no type to be checked against or
computed with. A `$` parameter is generic over its value and needs none, and a literal that is
the value of a declaration annotated with a function type takes the types of the parameters it
has in common with that type, and its result, from it, and so does a member of an impl, from the
requirement of its interface, and a closure written as an argument, from the call.

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

Or write it as a closure, which takes the types of its parameters from the call it is an
argument of:

```vun
twice := ($F: lang.ops.Call((u8), u8), f: F, v: u8) -> u8 { f(f(v)) };
four := twice([] (x) { (x * 2).wrap(u8) }, 1);
```
