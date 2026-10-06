# test.no_coverage

`vun cover` ran the tests' program and it wrote no coverage: it ended before its totals, or is a
`std` whose runner does not write the flags it was asked for (`--cover`).

The runner writes the flags of `lang.cover.map` the tests set just before the totals. A test
that ends the whole process, such as one calling `std.os.exit`, or a crash the runner cannot
survive, ends the program first, and there is nothing to report.

## Example

```console
$ vun cover app.vun
error[test.no_coverage]: the tests' program wrote no coverage
```

## Fix

Find the test that ended the program in the lines above the error, and run it with `vun test`.

```console
$ vun test app.vun
```
