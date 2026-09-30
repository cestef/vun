# `std.lints.style.unused`

A local binding that the code never names is reported: it is left over from an earlier version,
or another name is read where this one was meant.

```vun
area := (w: u8, h: u8) -> u16 { d := w * h; w * w };    // reported: the local `d` is never used
area := (w: u8, h: u8) -> u16 { d := w * h; d };
```

Parameters are not reported: they belong to the function's signature, which callers name and
interfaces fix.

Where a value is computed only for its effect, bind it to `_`, which discards it:

```vun
_ = compute();
```

The fix replaces the name with `_`. It is applied only on request (`vun fix --unsafe`): a value
bound to `_` ends where it is made, not at the end of the binding's scope.
