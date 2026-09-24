# meta.frozen

Reflection observes a type that is still being defined.

A type's member set is fixed when the expression defining it finishes evaluating; reflecting on a
type that is still being defined is an error naming the cycle (spec 03, section 6). This rules out
generators that observe their own output, which keeps impls and memoization sound.

## Example

```vun
Grow := struct {
    a: u8,
    size := std.meta.size(Grow); //~ meta.frozen
};
```

## Fix

Reflect on the type outside its definition, once it is complete:

```vun
Grow := struct {
    a: u8,
};

grow_size := std.meta.size(Grow);
```
