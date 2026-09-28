# types.arity

A call passes a number of arguments the function does not take.

A call passes one argument per parameter, the receiver of `a.f(b)` counting as the first. The
exceptions are inference (spec 02, section 1): the `$` parameters that a later parameter's type
names may all be left out, so such a function takes either every argument or every argument but
those; and a pack (spec 01, section 4), which takes the arguments after the others, as many as
its tuple type has items.

## Example

```vun
add := (a: u8, b: u8) -> u16 { a + b };

three := () -> u16 {
    add(1) //~ types.arity
};
```

## Fix

Pass one argument per parameter:

```vun
add := (a: u8, b: u8) -> u16 { a + b };

three := () -> u16 {
    add(1, 2)
};
```
