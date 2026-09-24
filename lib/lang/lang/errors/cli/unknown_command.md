# cli.unknown_command

A word in command position names no subcommand of the command before it.

`vun` groups its subcommands by what they act on (spec 12, section 7). The help suggests the
closest subcommand, or points at the group's `--help`, which lists them.

## Example

```console
$ vun chek blinky.vun
error[cli.unknown_command]: unknown command `chek` for `vun`
  │
  ╰ help: did you mean `check`?
```

## Fix

```console
$ vun check blinky.vun
```
