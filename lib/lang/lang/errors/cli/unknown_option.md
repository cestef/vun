# cli.unknown_option

A command is given an option it does not declare.

Each command declares its options, long (`--format`) and possibly short (`-h`), together with the
ones every command inherits (spec 12, section 7). The help suggests the closest long option, or
points at the command's `--help`, which lists them.

## Example

```console
$ vun check --formt=short blinky.vun
error[cli.unknown_option]: unknown option `--formt` for `vun check`
  │
  ╰ help: did you mean `--format`?
```

## Fix

```console
$ vun check --format=short blinky.vun
```
