# link.unavailable

The target's linker adapter cannot link yet, or is not in this toolchain.

Linkers are components registered with the toolchain behind adapters, each declaring its facts once
in a `LinkerInfo`; an adapter turns the target-independent `LinkPlan` into a linker script or flags
(spec 13, sections 7 and 8). A target names its linker (`.linker` in `lib/targets`: `gnu` for
Linux, `lld` for bare metal). An adapter that is registered but cannot link yet reports this
diagnostic instead of producing an output; today `gnu`, `mold` and `wild` link, through the
target's C compiler.

## Example

```console
$ vun build --target bare.cortex_m4f main.vun
error[link.unavailable]: linker `lld` cannot link yet
```

## Fix

Build for a target whose linker links, or change the target's `.linker` to one that does. `vun
toolchain components` lists the adapters compiled into this toolchain.

```console
$ vun toolchain components
```
