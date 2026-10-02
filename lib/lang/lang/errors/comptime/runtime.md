# comptime.runtime

A form needed when compiling, a `$` form that runs where it is written or the length of an
array, reads a local that the body computes when it runs.

`$e` asks for `e` at compile time (spec 03, section 7), and an array's size is part of its type.
A local bound with `:=` is computed when the program runs, even where its value is a constant, so
neither can read it: a `$assert` over it would check nothing, and an array would have no size. A
local bound with `$name :=` is known when compiling.

## Example

```vun
#[test]
floats := () {
    nan := 0.0 / 0.0;
    $assert(!(nan < 1.0)); //~ comptime.runtime
};
```

An array's length:

```vun
main := (sys: &mut std.os.System) -> std.os.Exit {
    n := 4;
    zeros := [0; n]; //~ comptime.runtime
    _ = zeros;
    .ok
};
```

## Fix

Bind the local at compile time, or check it when the program runs, without `$`:

```vun
#[test]
floats := () {
    $nan := 0.0 / 0.0;
    $assert(!(nan < 1.0));
};
```

```vun
main := (sys: &mut std.os.System) -> std.os.Exit {
    $n := 4;
    zeros := [0; n];
    _ = zeros;
    .ok
};
```
