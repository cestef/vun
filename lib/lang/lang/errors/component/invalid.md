# component.invalid

A component of the toolchain could not be loaded: its library cannot be opened, is not a
component, or was built for another release or protocol than the `vun` running.

A component is a backend or a linker kept as a dynamic library in the toolchain's `components`
directory (spec 12, section 6). Loading checks the library's descriptor against `vun`'s before
anything is called, so a component of another release is refused, never run.

## Example

```console
$ vun build hello.vun
error[component.invalid]: component `components/libvun_component_cranelift.so` is of release 0.3.0, not 0.4.2
```

## Fix

Install the components of the running toolchain, by updating it:

```console
$ vun toolchain update
```
