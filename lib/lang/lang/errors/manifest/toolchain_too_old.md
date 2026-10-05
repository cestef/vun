# manifest.toolchain_too_old

A package asks for a newer toolchain than the one reading its manifest.

`.requires` is the oldest toolchain that reads the package's manifest and builds it. It is read
before every other field: a package written for a newer toolchain may use fields this one does not
know, so reporting each of them as `manifest.unknown_field` would hide the cause. Only this
diagnostic is reported for the manifest. A toolchain older than the library it runs against meets
it in the packages of that library, a board for one.

## Example

```vun build.vun
package := .{
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .edition = 2026,
    .requires = "99.0.0", //~ manifest.toolchain_too_old "requires toolchain `99.0.0`"
    .include = .{ "build.vun", "blinky.vun" },
};

build := (b: &mut std.build.Build) {};
```

## Fix

Update the toolchain (`vun toolchain update`), or lower `.requires` to the oldest toolchain the
package builds with.

```vun build.vun
package := .{
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .edition = 2026,
    .requires = "0.1.0",
    .include = .{ "build.vun", "blinky.vun" },
};

build := (b: &mut std.build.Build) {};
```
