# comptime.unsupported

A `$` form that runs where it is written uses a form compile-time evaluation does not run yet.

`$e` asks for `e` at compile time, and so does the condition of a `$if` (spec 03, section 1).
Where the form surely runs, what it asks for cannot wait for run time: a `$` check that is not
run would pass without checking anything. Addresses of raw pointers are such a form: they exist
at run time only.

## Example

```vun
x: u8 = 5;

placed := () -> bool {
    $(lang.mem.address(u8, lang.mem.pointer_to(u8, &x)) != 0) //~ comptime.unsupported
};
```

## Fix

Compute it when the program runs, without `$`:

```vun
x: u8 = 5;

placed := () -> bool {
    lang.mem.address(u8, lang.mem.pointer_to(u8, &x)) != 0
};
```
