# toolchain.install_failed

A toolchain could not be installed: its release's manifest could not be read, the release has no
toolchain for this machine, its archive could not be fetched or unpacked, or the tree it unpacked
to does not hash to what the manifest names.

`vun toolchain install` and `update`, and a `vun` handing a pinned project to the toolchain it
pins, read the release's manifest (`release.json`), fetch the archive of the running machine's
toolchain, unpack it beside the installed ones and hash the tree before moving it in (spec 12,
section 6). Nothing is kept unless the hash matches. The cause says which step failed.

## Example

```console
$ vun toolchain install 0.3.0
error[toolchain.install_failed]: cannot install the toolchain 0.3.0
  caused by: the release has no toolchain for `riscv64` on `linux`
```

## Fix

Install a release that has a toolchain for this machine (`vun toolchain install` without a version
takes the latest), or build `vun` from source. A hash that does not match means the archive
changed after the release was made: install it again, and report it if it persists.

```console
$ vun toolchain install
```
