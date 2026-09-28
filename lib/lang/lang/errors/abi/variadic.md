# abi.variadic

A C function's pack holds what C cannot pass as variadic arguments.

Under `#[abi.c]`, the items of a function's pack are C's variadic arguments, its `...` (spec 07,
section 5): a C import is declared with the pack of the arguments its callers pass. Vun has no
way to read variadic arguments, so a C function with a pack has no body. C passes each variadic
argument as its default promotions leave it, so no item is an `f32` or an `f16`: C passes them
as `f64`.

## Example

```vun
#[link.symbol("printf"), abi.c]
printf: unsafe (format: *u8, ..args: .{ f32 }) -> i32 = _; //~ abi.variadic
```

## Fix

Pass the promoted type:

```vun
#[link.symbol("printf"), abi.c]
printf: unsafe (format: *u8, ..args: .{ f64 }) -> i32 = _;
```
