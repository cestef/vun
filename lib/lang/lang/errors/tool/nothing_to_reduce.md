# tool.nothing_to_reduce

`vun debug reduce` was given a file that does not fail as it was asked to keep.

A reduction cuts a file down to the least text that still fails the same way: the compiler
crashing at the same place, or reporting the same error. It starts from the file as it is, so the
file itself must fail that way: without `--keep`, by crashing or reporting an error; with
`--keep <id>`, by reporting that error.

## Example

```console
$ vun debug reduce --keep types.mismatch ok.vun
error[tool.nothing_to_reduce]: `ok.vun` does not report `types.mismatch`
```

## Fix

Check the file with `vun check` and name an error it reports, or leave out `--keep` to keep the
first failure the file has.

```console
$ vun check bad.vun
error[types.mismatch]: expected `bool`, found `Int(0..=255)`
$ vun debug reduce --keep types.mismatch bad.vun
     Reduced bad.vun from 171 to 18 bytes, 48 candidates tested
```
