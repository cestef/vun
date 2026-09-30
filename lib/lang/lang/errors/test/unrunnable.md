# test.unrunnable

The program running a namespace's tests could not be made: `std.testing.runner` did not give it
at compile time.

`vun test` calls `std.testing.runner` with the namespace at compile time, and builds the program
whose exports are the members of the type it gives (spec 11, section 1). A trap while it runs is
reported as `comptime.trap` instead; this error is for the rest: a `std` whose `testing.runner`
gives no type, or a form the evaluator does not run yet.

## Example

```console
$ vun test app.vun
error[test.unrunnable]: the tests could not be collected: it uses a form compile-time evaluation does not run yet
```

## Fix

Build with the `std` of the toolchain, whose `testing.runner` gives a program's exports.

```console
$ vun test app.vun
      Passed crc_vector
      Tested 1 passed, 0 trapped
```
