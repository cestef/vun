# backend.tool

A tool a build step runs, such as the C compiler, could not be started or reported failure.

Backends that emit text hand it to a tool (spec 13, section 1): the C backend's output goes to
the C compiler of the toolchain profile, which also links hosted programs. The diagnostic names
the tool and carries what it printed.

## Example

```console
$ vun build hello.vun
error[backend.tool]: `cc` could not be started
  caused by: No such file or directory (os error 2)
```

## Fix

Install the tool, or make it reachable on `PATH`. When the C compiler rejects the code the C
backend wrote, that is a bug in the backend: report it with the output of `vun debug c`.

```console
$ vun debug c hello.vun > hello.c
$ cc -std=c99 -fno-strict-aliasing -c hello.c
```
