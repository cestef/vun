# cli.missing_value

An option that takes a value is given none.

An option with a value is written `--name value` or `--name=value` (spec 12, section 7); nothing
follows it here.

## Example

```console
$ vun check blinky.vun --diagnostics
error[cli.missing_value]: option `--diagnostics` needs a value
  │
  ╰ help: write `--diagnostics <diagnostics>`
```

## Fix

```console
$ vun check blinky.vun --diagnostics short
```
