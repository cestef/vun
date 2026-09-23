# cli.unexpected_argument

A command is given more positional arguments than it declares.

Each command declares its arguments (spec 12, section 7); a word left over after them is not
ignored. `overt check` takes one file.

## Example

```console
$ overt check blinky.ov board.ov
error[cli.unexpected_argument]: unexpected argument `board.ov` for `overt check`
  │
  ╰ help: see `overt check --help`
```

## Fix

Run the command once per argument:

```console
$ overt check blinky.ov
$ overt check board.ov
```
