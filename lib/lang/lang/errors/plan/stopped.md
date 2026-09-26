# plan.stopped

The package's `build` could not run to its end at compile time, so the package has no plan.

The driver runs `build` in the compile-time evaluator, through `std.build.plan`, and reads the
plan it records (spec 12, section 2). Like every compile-time call, it runs only what compile-time
evaluation supports and reads only values known at compile time (spec 03, section 7). A trap
while it runs is reported as `comptime.trap` instead; this error is for the rest: a form the
evaluator does not run yet, or a value known only at run time.

## Example

A `mut` top-level binding is a static: what it holds is known at run time only.

```vun build.vun
package := .{
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .edition = 2026,
    .include = .{ "build.vun" },
};

mut boards: u8 = 1;

build := (b: &mut std.build.Build) { //~ plan.stopped "it reads a value known only at run time"
    b.lints(.{ .boards = unsafe { boards } });
};
```

## Fix

Read only what is known at compile time: here, a binding that is not `mut`.

```vun build.vun
package := .{
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .edition = 2026,
    .include = .{ "build.vun" },
};

boards: u8 = 1;

build := (b: &mut std.build.Build) {
    b.lints(.{ .boards = boards });
};
```
