# tool.bind_skipped

A C declaration was left out of the bindings, or bound without its fields.

`vun util ffi bind` binds what Vun can say exactly and never approximates the rest (spec 07,
section 5):

- a function taking `...` is left out;
- a union, or a struct with a bit field or a field Vun cannot say, is bound as its size and
  alignment only, private bytes, so that it can still be a field of another struct; passed by
  value, its fields would matter to the calling convention, so a function doing so is left out;
- a type Vun has no counterpart for, such as `long double`, leaves out what uses it.

## Example

```console
$ vun util ffi bind sensor.h
warning[tool.bind_skipped]: `log_line` is left out: it takes arguments past its parameters (`...`)
```

## Fix

Declare what is needed by hand, next to the bindings: a function of fixed parameters wrapping the
variadic one in a C file of the build, or the struct's fields as the C code lays them out.

```vun
/// `void log_line_str(const char *text)`, a C wrapper of `log_line(fmt, ...)` in the build.
#[lang.link.symbol("log_line_str"), lang.abi.c]
log_line_str: unsafe (text: ?*lang.int.signed(8)) = _;
```
