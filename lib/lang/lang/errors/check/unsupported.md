# check.unsupported

The program is built with checks of unsafe code that its runtime does not provide.

A check calls operations of `lang.check` that the program's runtime exports as link symbols,
as it exports `lang.panic.trap` (spec 07, section 7). A runtime that exports none of them, or not
those a check needs, cannot run a checked program, so linking stops and names the check and the
missing operation.

## Example

```console
$ vun build --checks pointers --target wasi.wasm32 main.vun
error[check.unsupported]: the runtime of the program does not provide `lang.check.access`, which its checks need: `pointers`
```

## Fix

Build without the check, or for a runtime that provides it: the runtimes of `std.os` over a C
library do (`std.check.exports`).

```console
$ vun build --checks pointers main.vun
```
