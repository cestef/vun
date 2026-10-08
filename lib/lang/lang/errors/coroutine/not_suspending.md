# coroutine.not_suspending

A function returns an interface but never yields.

A function whose body yields is a coroutine: it returns its frame, a value of a type that
implements the interface the result names, such as `Iter(u32)` (spec 06, section 1). Without a
`yield` the function is ordinary, and an interface is not a type an ordinary function can
return.

## Example

```vun
none := () -> Iter(u32) { //~ coroutine.not_suspending
    ()
};
```

## Fix

Yield from the body, or return a type:

```vun
some := () -> Iter(u32) {
    yield 1;
};
```
