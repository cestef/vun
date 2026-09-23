# cli.missing_argument

A command is run without one of its positional arguments.

Each command declares its arguments (spec 12, section 7); the help gives its usage, and `--help`
describes each argument.

## Example

```console
$ overt check
error[cli.missing_argument]: `overt check` needs `<file>`: the source file to read
  │
  ╰ help: usage: `overt check [options] <file>`
```

## Fix

```console
$ overt check blinky.ov
```
