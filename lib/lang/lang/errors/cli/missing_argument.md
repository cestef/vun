# cli.missing_argument

A command is run without one of its positional arguments.

Each command declares its arguments (spec 12, section 7); the help gives its usage, and `--help`
describes each argument.

## Example

```console
$ vun check
error[cli.missing_argument]: `vun check` needs `<file>`: the root source file
  │
  ╰ help: usage: `vun check [options] <file>`
```

## Fix

```console
$ vun check blinky.vun
```
