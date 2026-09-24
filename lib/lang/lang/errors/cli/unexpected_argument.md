# cli.unexpected_argument

A command is given more positional arguments than it declares.

Each command declares its arguments (spec 12, section 7); a word left over after them is not
ignored. `vun check` takes one file.

## Example

```console
$ vun check blinky.vun board.vun
error[cli.unexpected_argument]: unexpected argument `board.vun` for `vun check`
  │
  ╰ help: see `vun check --help`
```

## Fix

Run the command once per argument:

```console
$ vun check blinky.vun
$ vun check board.vun
```
