# link.unavailable

The linker a build takes cannot link yet, or is not in this toolchain.

Linkers are components registered with the toolchain behind adapters, each declaring its facts once
in a `LinkerInfo`; an adapter turns the target-independent `LinkPlan` into a linker script or flags
(spec 13, sections 7 and 8). A target names its linker (`.linker` in `lib/targets`: `gnu` for
Linux and bare metal, `lld` for macOS, `wasm` for WASI), and `--linker` names another. An adapter that is
registered but cannot link yet reports this diagnostic instead of producing an output, and so does
one whose program is not on the search path (`wild`, `wasm-ld`, `ld64.lld`).

## Example

```console
$ vun build --linker external main.vun
error[link.unavailable]: linker `external` cannot link yet
```

## Fix

Name a linker that links, or none: the target's then links. `vun toolchain show` lists the
linkers compiled into this toolchain, and `vun target show` which one a build takes, why, and
whether the machine has the programs it runs.

```console
$ vun target show --linker external main.vun
```
