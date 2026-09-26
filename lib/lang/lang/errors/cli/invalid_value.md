# cli.invalid_value

An argument or an option's value is not one the command accepts.

Parameters declare the values they take (spec 12, section 7): `--diagnostics` takes one of the
diagnostic formats, `--color` one of `auto`, `always` and `never`, `vun explain` a declared
diagnostic, `--version` of `vun pkg add` a version `major.minor.patch`. Declared choices are
checked while the command line is read; the help suggests the closest, or lists them.

## Example

```console
$ vun check blinky.vun --diagnostics=shrt
error[cli.invalid_value]: `shrt` is not a value of `--diagnostics`
  │
  ╰ help: did you mean `short`?
```

## Fix

```console
$ vun check blinky.vun --diagnostics=short
```
