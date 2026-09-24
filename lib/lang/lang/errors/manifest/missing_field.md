# manifest.missing_field

A manifest's `package` literal lacks a field that every package declares.

A directory holds a package when its `build.vun` declares `package` (spec 12, section 2). Tools
read that declaration as a literal, without evaluating anything, so the fields it needs are written
out: `.name`, `.id`, `.version`, `.edition` and `.include`.

## Example

```vun build.vun
package := .{ //~ manifest.missing_field
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .include = .{ "build.vun", "blinky.vun" },
};

build := (b: &mut std.build.Build) {};
```

## Fix

Declare the missing field; `.edition` is the edition the package is written in:

```vun build.vun
package := .{
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .edition = 2026,
    .include = .{ "build.vun", "blinky.vun" },
};

build := (b: &mut std.build.Build) {};
```
