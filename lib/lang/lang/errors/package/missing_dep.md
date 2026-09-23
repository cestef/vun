# package.missing_dep

A dependency is not on disk where it is declared.

Each dependency is a binding in scope under its declared name (spec 01, section 8). A `.path`
dependency is the package in that directory, which must hold a `build.ov` declaring `package`; an
archive dependency is the entry of its hash in the store, which `overt pkg fetch` fills (spec 12,
section 3). The dependency is looked for when one of its namespaces is needed, here through the
prelude.

## Example

```ov build.ov
package := .{
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .edition = 2026,
    .include = .{ "build.ov", "blinky.ov" },
    .prelude = .{ "std.prelude", "board" },
    .deps = .{
        .board = .{ .path = "board" }, //~ package.missing_dep
    },
};
```

## Fix

Point `.path` at the directory of the package, or create it there:

```ov build.ov
package := .{
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .edition = 2026,
    .include = .{ "build.ov", "blinky.ov" },
    .prelude = .{ "std.prelude", "board" },
    .deps = .{
        .board = .{ .path = "board" },
    },
};
```

```ov board/build.ov
package := .{
    .name = "board",
    .id = 2,
    .version = "0.1.0",
    .edition = 2026,
    .include = .{ "build.ov", "board.ov" },
};
```

```ov board/board.ov
led := 13;
```
