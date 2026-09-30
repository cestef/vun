# link.duplicate

Two objects define the same symbol, or a library's function is imported with two types.

A symbol that objects share is defined once. A weak definition gives way to a strong one; two
strong definitions of one name are refused, as is one function of a library imported under one
name with two different types, which the program could not both call.

## Example

```console
$ vun build --target wasi.wasm32.p1 main.vun
error[link.duplicate]: cannot link `main.wasm`
  caused by: `counter` is defined twice
```

## Fix

Keep one definition, or make the others local to their object.

```console
$ vun build --target wasi.wasm32.p1 main.vun
```
