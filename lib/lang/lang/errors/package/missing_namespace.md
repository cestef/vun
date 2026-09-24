# package.missing_namespace

A path that must name a namespace names nothing on disk.

A spread `..path;` and an entry of the manifest's `.prelude` name a namespace by a path that
starts at the package or one of its dependencies (spec 01, section 8), so tools resolve them from
the manifest and the files alone. Each further name is a file `name.vun` or a directory `name/`
below the previous one.

## Example

```vun build.vun
package := .{
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .edition = 2026,
    .include = .{ "build.vun", "blinky.vun", "blinky" },
};

build := (b: &mut std.build.Build) {};
```

```vun blinky.vun
..blinky.pins; //~ package.missing_namespace
```

## Fix

Create the namespace, here the file `blinky/pins.vun`, or spread one that exists:

```vun build.vun
package := .{
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .edition = 2026,
    .include = .{ "build.vun", "blinky.vun", "blinky" },
};

build := (b: &mut std.build.Build) {};
```

```vun blinky.vun
..blinky.pins;
```

```vun blinky/pins.vun
led := 13;
```
