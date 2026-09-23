# cli.unknown_command

A word in command position names no subcommand of the command before it.

`overt` groups its subcommands by what they act on (spec 12, section 7). The help suggests the
closest subcommand, or points at the group's `--help`, which lists them.

## Example

```console
$ overt chek blinky.ov
error[cli.unknown_command]: unknown command `chek` for `overt`
  │
  ╰ help: did you mean `check`?
```

## Fix

```console
$ overt check blinky.ov
```
