# types.arity

A call passes a number of arguments the function does not take.

A call passes one argument per parameter, the receiver of `a.f(b)` counting as the first. The one
exception is inference (spec 02, section 1): the `$` parameters that a later parameter's type
names may all be left out, so such a function takes either every argument or every argument but
those.

## Example

```ov
add := (a: u8, b: u8) -> u16 { a + b };

three := () -> u16 {
    add(1) //~ types.arity
};
```

## Fix

Pass one argument per parameter:

```ov
add := (a: u8, b: u8) -> u16 { a + b };

three := () -> u16 {
    add(1, 2)
};
```
