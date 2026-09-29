# comptime.export

An exported declaration whose value has none at compile time.

A declaration exported as data (`#[link.export(s)]` on a value that is not a function, such as a
vector table) is placed in the program as the bytes of its value, which compile-time evaluation
computes (spec 08, section 2). A value that reads what exists only at run time, calls a function
provided from outside, or calls a function whose body does not build has no bytes to place.
Where evaluation stopped inside a function it calls, the report points there too.

## Example

```vun
#[link.symbol("now")]
now: () -> u32 = _;

#[link.export("started")] started: u32 = now(); //~ comptime.export
```

## Fix

Give it a value known when compiling, and compute the rest when the program runs:

```vun
#[link.export("started")] started: u32 = 0;
```
