# cli.invalid_value

An argument or an option's value is not one the command accepts.

Some values are checked against what they name (spec 12, section 7): `--format` takes one of the
diagnostic formats, `vun explain` a declared diagnostic, `--version` of `vun pkg add` a version
`major.minor.patch`. The help suggests the closest accepted value, or lists them.

## Example

```console
$ vun check blinky.vun --format=shrt
error[cli.invalid_value]: unknown format `shrt`
  │
  ╰ help: did you mean `short`?
```

## Fix

```console
$ vun check blinky.vun --format=short
```
