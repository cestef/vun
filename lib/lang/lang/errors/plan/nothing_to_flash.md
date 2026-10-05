# plan.nothing_to_flash

`vun flash` found no program of the plan that is written to a device.

A program is flashed by a step of the plan, `b.flash(program, method)` (spec 12, section 2), next
to the step that builds it.

## Example

```console
$ vun flash
error[plan.nothing_to_flash]: the plan flashes no program
  help: record one with `b.flash(program, .uf2(.{ .family = ... }))`
```

## Fix

Add the step to the package's `build`, after the program it writes.

```vun
build := (b: &mut std.build.Build) {
    firmware := b.program(.{ .root = "app.vun", .target = targets.bare.thumbv8m_main.eabihf });
    b.flash(firmware, .uf2(.{ .family = 0xe48bff59 }));
};
```
