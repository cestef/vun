# toolchain.prime_failed

The toolchain could not be primed: a run over one of its templates failed, or the memos could not
be written.

Priming runs the toolchain's own executable over each template of its library, from an empty
store, and keeps the libraries' memos those runs saved in the toolchain's `memos` directory. A
run that ends without success, or runs past the limit, stops it; the cause follows the diagnostic
as a note.

## Example

```console
$ vun toolchain prime
error[toolchain.prime_failed]: cannot prime the toolchain
  = note: `check` of the template hello ended with exit status: 1
```

## Fix

Check the template the note names with `vun check`, or prime over the templates that work:

```console
$ vun toolchain prime hello
```
