# package.missing_namespace

A path that must name a namespace names nothing on disk.

A spread `..path;` and an entry of the manifest's `.prelude` name a namespace by a path that
starts at the package or one of its dependencies (spec 01, section 8), so tools resolve them from
the manifest and the files alone. Each further name is a file `name.ov` or a directory `name/`
below the previous one.

## Example

```ov build.ov
package := .{
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .edition = 2026,
    .include = .{ "build.ov", "blinky.ov", "blinky" },
};
```

```ov blinky.ov
..blinky.pins; //~ package.missing_namespace
```

## Fix

Create the namespace, here the file `blinky/pins.ov`, or spread one that exists:

```ov build.ov
package := .{
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .edition = 2026,
    .include = .{ "build.ov", "blinky.ov", "blinky" },
};
```

```ov blinky.ov
..blinky.pins;
```

```ov blinky/pins.ov
led := 13;
```
