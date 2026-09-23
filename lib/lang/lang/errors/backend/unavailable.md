# backend.unavailable

The selected backend cannot generate code yet.

Backends are components registered with the toolchain, each declaring its facts once in a
`BackendInfo`; nothing above LIR knows which one runs (spec 13, sections 7 and 8). A target names
its default backend (`.default_backend` in `lib/targets`: `cranelift` for x86_64, `c` for
thumbv7em). A backend that is registered but cannot generate code yet reports this diagnostic
instead of emitting anything. In this toolchain every backend is still a stub, and `overt build`,
the command that runs them, is planned: today it reports `cli.unavailable_command`.

## Example

Once `overt build` exists, building for x86_64 reaches the Cranelift backend:

```console
$ overt build
error[backend.unavailable]: backend `cranelift` cannot generate code yet
```

## Fix

Select a backend that generates code for the target; `overt components` lists the backends
compiled into this toolchain, none of which generates code yet:

```console
$ overt components
backends: c, cranelift, qbe, llvm, process
linkers:  wild, mold, lld, gnu, msvc, process
```
