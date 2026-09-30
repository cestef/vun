# toolchain.not_installed

The command names a toolchain version that is not installed.

Installed toolchains are directories named by their version beside each other (spec 12, section
6); `vun toolchain list` lists them.

## Example

```console
$ vun toolchain remove 0.1.0
error[toolchain.not_installed]: the toolchain 0.1.0 is not installed
```

## Fix

Name a version `vun toolchain list` shows, or install it first with `vun toolchain install`.

```console
$ vun toolchain list
$ vun toolchain install 0.1.0
```
