# lint.unfulfilled

A lint expected in a package did not report.

At the level `expect` a lint warns when it fires and, just as much, when it does not: an
expectation left unfulfilled is reported so that it does not silently stop guarding what it was
written for (spec 10, section 4).

## Example

A package expects a lint in its build, and nothing in the file would report it:

```console
$ vun check .
warning[lint.unfulfilled]: expected `std.lints.style.unused` to report, and it did not
  ╰ help: remove the expectation, or fix what silences the lint
```

## Fix

Remove the expectation, or fix what silences the lint it expects:

```console
$ vun check .
    Finished . in 0.1s
```
