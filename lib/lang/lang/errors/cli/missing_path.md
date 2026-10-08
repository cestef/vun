# cli.missing_path

A command was given a path that names no file or directory.

Commands that act on a file, a directory or a package (spec 12, section 7) read the path before
they begin, so a typo is reported at once and nothing is built, formatted or tested for it. The
help suggests the closest name in the directory the path is in.

## Example

```console
$ vun fmt src/mian.vun
error[cli.missing_path]: `src/mian.vun` does not exist
  │
  ╰ help: did you mean `src/main.vun`?
```

## Fix

Name a file or directory that exists:

```console
$ vun fmt src/main.vun
```
