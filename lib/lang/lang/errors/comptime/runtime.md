# comptime.runtime

A `$` form that runs where it is written reads a local that the body computes when it runs.

`$e` asks for `e` at compile time (spec 03, section 7). A local bound with `:=` is computed when
the program runs, even where its value is a constant, so a `$` form cannot read it: a `$assert`
over it would check nothing. A local bound with `$name :=` is known when compiling.

## Example

```vun
#[test]
floats := () {
    nan := 0.0 / 0.0;
    $assert(!(nan < 1.0)); //~ comptime.runtime
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
