# tool.bind_failed

`vun tool bind` could not read the header.

Bindings are written only from a header clang reads without errors, with the flags the build
compiles it with: a missing include directory or macro would otherwise bind the wrong
declarations, or the wrong sizes (spec 07, section 5). The errors clang reported follow the
diagnostic as notes.

## Example

```console
$ vun tool bind sensor.h
error[tool.bind_failed]: clang could not read `sensor.h`
  = note: sensor.h:1:10: fatal error: 'board.h' file not found
```

## Fix

Pass the build's compile flags, such as include directories and definitions:

```console
$ vun tool bind sensor.h --cflags "-Ivendor/include -DBOARD_REV=2"
```
