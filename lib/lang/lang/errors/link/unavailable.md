# link.unavailable

The selected linker adapter cannot link yet.

Linkers are components registered with the toolchain behind adapters, each declaring its facts once
in a `LinkerInfo`; an adapter turns the target-independent `LinkPlan` into a linker script or flags
(spec 13, sections 7 and 8). A target names its default linker (`.default_linker` in
`lib/targets`: `mold` for x86_64, `lld` for thumbv7em). An adapter that is registered but cannot
link yet reports this diagnostic instead of producing an output. In this toolchain every adapter is
still a stub, and `vun build`, the command that runs them, is planned: today it reports
`cli.unavailable_command`.

## Example

Once `vun build` exists, building for x86_64 reaches the mold adapter:

```console
$ vun build
error[link.unavailable]: linker `mold` cannot link yet
```

## Fix

Select a linker that can link for the target; `vun components` lists the adapters compiled into
this toolchain, none of which links yet:

```console
$ vun components
backends: c, cranelift, qbe, llvm, process
linkers:  wild, mold, lld, gnu, msvc, process
```
