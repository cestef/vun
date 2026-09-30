# test.no_namespace

The path `vun test` was given is no namespace of a package, so it names no tests.

`vun test` runs the tests of a namespace (spec 11, section 1): a file, a directory, or a package's
directory, which is its root namespace; with no path, the package around the current directory.
A file or directory outside the package's files, such as one its `include` leaves out, is no
namespace of it, and a file outside any package is in no namespace a test
runner can collect.

## Example

```console
$ vun test notes/
error[test.no_namespace]: `notes/` is no namespace of a package
```

## Fix

Name a file or directory of the package, or its directory:

```console
$ vun test src/
```
