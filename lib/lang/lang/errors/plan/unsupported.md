# plan.unsupported

A step of the package's plan is recorded, but this toolchain does not carry it out yet.

`vun build` on a `build.vun` carries out its plan in order (spec 12, section 2): it builds each
program the plan records. Writing a program to its device (`b.flash`), running its tests on a
runner (`b.test_on`), passing parameters to a dependency (`b.dep` with a record that is not
empty) and lint levels (`b.lints`) are recorded and checked, but not carried out yet. This is a
warning: the programs are still built.

## Example

```console
$ vun build build.vun
warning[plan.unsupported]: step 2 of the plan writes a program to its device, which this toolchain does not do yet
```

## Fix

Nothing is required. To silence it, leave the step out of `build` until the toolchain carries it
out:

```console
$ vun build build.vun
    Finished build.vun
```
