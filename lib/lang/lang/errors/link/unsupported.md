# link.unsupported

The linker named does not link the target's objects.

Each linker adapter declares the object formats it links and whether it places sections in
memory regions (`LinkerInfo`, spec 13, section 7): `mold` and `wild` link ELF, `gnu` ELF with
linker scripts. A build naming a linker with `--linker` for a target outside them is refused.

## Example

```console
$ vun build --linker mold --target macos.arm64 main.vun
error[link.unsupported]: linker `mold` does not link objects of format `macho`
```

## Fix

Name a linker that links the target's format, or none: the target's own linker then links.

```console
$ vun build --target macos.arm64 main.vun
```
