# plan.unread

A step of the package's plan is not a `std.build.Action` as `std.build` declares it.

`vun build` on a `build.vun` reads each step its `build` records, member by member, as the
declarations of `std.build` give them (spec 12, section 2). A step that does not read so comes from
a `std.build` another toolchain declares differently.

## Example

```console
$ vun build build.vun
error[plan.unread]: step 1 of the plan is not a `std.build.Action`: the member `std.build.Options.target` is not a literal of its type
```

## Fix

Record steps only through the methods of `std.build.Build`, with the `std` of the toolchain that
builds the package:

```console
$ vun build build.vun
    Finished build.vun
```
