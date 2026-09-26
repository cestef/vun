# cli.unknown_option

A command is given an option it does not declare.

Each command declares its options, long (`--diagnostics`) and possibly short (`-h`), together
with the sets it shares with other commands and the global ones (spec 12, section 7). The help suggests the closest long option, or
points at the command's `--help`, which lists them.

## Example

```console
$ vun check --diagnostic=short blinky.vun
error[cli.unknown_option]: unknown option `--diagnostic` for `vun check`
  │
  ╰ help: did you mean `--diagnostics`?
```

## Fix

```console
$ vun check --diagnostics=short blinky.vun
```
