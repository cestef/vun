# backend.unsupported

The backend named does not generate code for the target's architecture.

Each backend declares the architectures it generates code for (`BackendInfo.arches`, spec 13,
section 7): the native backend generates x86-64, the C backend any architecture with a C
compiler. A build naming a backend with `--backend` for a target outside them is refused.

## Example

```console
$ vun build --backend native --target bare.thumbv7em.eabihf main.vun
error[backend.unsupported]: backend `native` does not generate code for `arm`
```

## Fix

Name a backend that generates code for the architecture, or none: the target's preferences for
the profile then choose one that does.

```console
$ vun build --target bare.thumbv7em.eabihf main.vun
```
