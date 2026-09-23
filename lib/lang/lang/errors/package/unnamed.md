# package.unnamed

`overt pkg add` adds a package that does not state its name or its version.

A dependency is declared under a name, and an archive dependency with its version (spec 12,
section 3). Both come from the package's `build.ov`; a foreign archive has none, such as a C
library or a vendor SDK, so they are given on the command line with `--name` and `--version`.

## Example

```console
$ overt pkg add file://sdk-2.0.0.tar.gz
error[package.unnamed]: `file://sdk-2.0.0.tar.gz` has no `build.ov` stating its name
  │
  ╰ help: give it with `--name <name>`
$ overt pkg add file://sdk-2.0.0.tar.gz --name sdk
error[package.unnamed]: `file://sdk-2.0.0.tar.gz` has no `build.ov` stating its version
  │
  ╰ help: give it with `--version <version>`
```

## Fix

Give both; the archive is declared as `.kind = .foreign`:

```console
$ overt pkg add file://sdk-2.0.0.tar.gz --name sdk --version 2.0.0
added sdk 2.0.0 blake3:3gnwl2eucypkmcykdtk5qgts2f6cfsp322z3ewh3jlqkplyu355q
```
