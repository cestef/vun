# component.failed

A component stopped while it worked: it panicked, or answered with bytes that are not a reply.

The component's own diagnostics are reported as they are; this one says the component itself
failed, with the panic's message or what was malformed. It is a bug in the component: report it
with the output of `VUN_LOG=vun_component=debug`, which names each call into the component.

## Example

```console
$ vun build hello.vun
error[component.failed]: component `cranelift` panicked: index out of bounds
```

## Fix

Build with another backend, and report the program:

```console
$ vun build --backend native hello.vun
```
