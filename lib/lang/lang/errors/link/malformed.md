# link.malformed

An input is not a relocatable object the wasm linker reads.

The wasm linker reads objects in the WebAssembly tool conventions' format, as the wasm backend
and clang write them: a module with a `linking` section of version 2 and `reloc.*` sections. It
refuses bytes that are not a wasm module, a module that is not relocatable, and what it does not
link yet: tables, memories or globals the object defines itself, passive data segments,
initialization functions and relocation kinds past those listed in spec 13, section 8.4.

## Example

```console
$ vun build --target wasi.wasm32.p1 main.vun
error[link.malformed]: cannot read the object `c/helper.o`
  caused by: it has no `linking` section of version 2: it is not relocatable
```

## Fix

Compile the object with relocations (`clang -c`), or link with `--linker lld`.

```console
$ vun build --target wasi.wasm32.p1 --linker lld main.vun
```
