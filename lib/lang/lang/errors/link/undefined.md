# link.undefined

A symbol the program uses is defined by none of its objects and provided by no library.

The wasm linker resolves each symbol an object leaves undefined against the others' definitions.
A function declared with a library (`#[link.library(name)]`, spec 08) stays an import of the
program from that library; any other undefined symbol must be defined by another object, or be
one the linker defines itself (`__stack_pointer`, `__indirect_function_table`, `__data_end`,
`__heap_base`). A symbol defined as another kind than it is used as (data called as a function)
is undefined too. The entry symbol of the target (`_start` on WASI) must be a function the
program defines.

## Example

```console
$ vun build --target wasi.wasm32.p1 main.vun
error[link.undefined]: cannot link `main.wasm`
  caused by: `helper` is defined nowhere
```

## Fix

Link the object that defines the symbol, or declare the library that provides it
(`#[link.library(name)]`), whose import the program then keeps.

```console
$ vun build --target wasi.wasm32.p1 main.vun
```
