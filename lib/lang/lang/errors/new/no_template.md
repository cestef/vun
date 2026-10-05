# new.no_template

`vun new` was asked for a template this toolchain does not have.

Templates are the packages in the `templates` directory of the toolchain's library, each a
project that builds as it is. `vun new` copies one and gives the copy its name and a package id of
its own.

## Example

```console
$ vun new blinky --template pico
error[new.no_template]: no template `pico` in the library
  help: did you mean `rp2350`?
```

## Fix

Name one of the templates the error lists, or omit the option to start from the default.

```console
$ vun new blinky --template rp2350
    Creating blinky
```
