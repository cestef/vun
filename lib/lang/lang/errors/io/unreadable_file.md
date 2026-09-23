# io.unreadable_file

A file the command needs cannot be read.

The file system's reason is kept as the chain of causes (spec 10, section 2): a missing file, a
missing permission, text that is not UTF-8. It arises for the file a command is given, and for the
files a package includes when `overt pkg pack` archives them.

## Example

```console
$ overt check blinky/main.ov
error[io.unreadable_file]: cannot read `blinky/main.ov`
  │
  ╰ caused by: No such file or directory (os error 2)
```

## Fix

Name a file that exists and can be read:

```console
$ overt check blinky.ov
```
