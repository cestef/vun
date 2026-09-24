# plan.missing_build

A `build.vun` that declares `package` does not declare what `std.build.Package` requires of it.

A package's `build.vun` conforms to the interface `std.build.Package` (spec 12, section 2). The
interface requires `build`, the function that records the package's plan on the
`&mut std.build.Build` it is lent. The driver runs it to learn what to build; without it, the
package has no plan.

## Example

```vun build.vun
package := .{ //~ plan.missing_build "does not declare `build`"
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .edition = 2026,
    .include = .{ "build.vun" },
};
```

## Fix

Declare `build`, even when the package plans nothing:

```vun build.vun
package := .{
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .edition = 2026,
    .include = .{ "build.vun" },
};

build := (b: &mut std.build.Build) {};
```
