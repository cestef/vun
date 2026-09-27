# check.unsupported

The program is built with checks of unsafe code that its runtime does not provide.

A check calls operations of `lang.check` that the program's runtime exports as link symbols,
as it exports `lang.panic.trap` (spec 07, section 7). A runtime that exports none of them, or not
those a check needs, cannot run a checked program, so linking stops and names the check and the
missing operation.

## Example

```console
$ vun build --checks pointers blink.vun
error[check.unsupported]: the runtime of `blink.vun` does not provide `lang.check.access`, which `pointers` needs
```

## Fix

Build without the check, or with a runtime that provides it: the hosted runtimes of `std.os`
do, and a board runtime does once `Options.memory` gives it a `.shadow` region.

```console
$ vun build blink.vun
```
