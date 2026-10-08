# toolchain.no_component

The command names a component the toolchain's release does not build for this machine.

A component is named by its kind and id, `backend-cranelift` or `linker-wasm`; a bare id such as
`cranelift` is taken when only one kind has it. `vun component list` lists what the release
builds (spec 12, section 6).

## Example

```console
$ vun component add cranelit
error[toolchain.no_component]: the release builds no component `cranelit`
  │
  ╰ help: did you mean `backend-cranelift`?
```

## Fix

```console
$ vun component list
$ vun component add backend-cranelift
```
