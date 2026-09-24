# backend.lacks

The program needs something the selected backend cannot express.

Capabilities are checked, never faked (spec 13, section 7): a backend declares what it can
generate, and a program needing more is refused rather than given an approximation. The C
backend writes strict C99 (spec 13, section 8.2), which has no 128-bit integers, no `f16`, no
atomics, no guaranteed tail calls, no naked functions and no link sections; it also needs every
link symbol to be a C identifier that is not a keyword or a name its headers declare, and every
alignment to be one an integer type has.

## Example

```console
$ vun build --backend c counter.vun
error[backend.lacks]: backend `c` cannot express atomic memory access
```

## Fix

Build with a backend that has the capability, or write the program without it. A link symbol
can be renamed with `#[link.export("name")]` or `#[link.symbol("name")]`.

```console
$ vun build --backend llvm counter.vun
```
