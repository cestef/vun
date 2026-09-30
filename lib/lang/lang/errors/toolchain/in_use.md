# toolchain.in_use

The toolchain cannot be removed: it is the default one, which the user's `vun` links to, or the one
running the command.

## Example

```console
$ vun toolchain remove 0.4.2
error[toolchain.in_use]: the toolchain 0.4.2 is the default
```

## Fix

Make another toolchain the default first, then remove this one.

```console
$ vun toolchain install 0.5.0 --default
$ vun toolchain remove 0.4.2
```
