# cli.missing_value

An option that takes a value is given none.

An option with a value is written `--name value` or `--name=value` (spec 12, section 7); nothing
follows it here.

## Example

```console
$ overt check blinky.ov --format
error[cli.missing_value]: option `--format` needs a value
  │
  ╰ help: write `--format <format>`
```

## Fix

```console
$ overt check blinky.ov --format short
```
