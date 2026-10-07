# flow.tail_position

A guaranteed tail call is not the value of a `return`, or is in a coroutine function.

`lang.flow.tail(f, args)` replaces the frame of the function running it by a call of `f`, so
nothing of the caller can run after it: it must be written as `return lang.flow.tail(f, args)`
(spec 09, section 3).

## Example

```vun
count := (n: u32) -> u32 {
    if n == 0 { return 0; }
    lang.flow.tail(count, (n - 1,)) //~ flow.tail_position
};
```

A coroutine function returns through its frame, which a jump would skip, so it has none.

## Fix

Return the call:

```vun
count := (n: u32) -> u32 {
    if n == 0 { return 0; }
    return lang.flow.tail(count, (n - 1,));
};
```
