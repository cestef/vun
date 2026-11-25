---
title: "Packages"
description: "`build.vun`, the package record, dependencies and archives."
---

A package is a directory with a `build.vun` at its root. The file is Vun, so a package is described in the language it is written in.

## The record

```vun
package := .{
    .name = "hello",
    .id = 13712801570853811626,
    .version = "0.1.0",
    .edition = 2026,
    .include = .{ "build.vun", "main.vun", "hello.vun", "hello" },
    .deps = .{},
};
```

| Field | Is |
|---|---|
| `name` | the package's name, as other packages import it |
| `id` | a number that identifies the package, made by `vun new` |
| `version` | the release, `major.minor.patch` |
| `edition` | the edition of the language the package is written for |
| `include` | the files and directories that belong to the package and its archive |
| `deps` | the dependencies, by the name they are imported under |

## The build

```vun
build := (b: &mut std.build.Build) {
    b.program(.{ .root = "main.vun", .target = lang.target.current });
};
```

`build` is a function over `std.build.Build`. `b.program` declares a program: its root file and the target it builds for. `vun build` and `vun run` run this function to learn what the package makes, so a build is code you can read and test, with no second language for it.

## Dependencies

```sh
vun pkg add ../lib1
```

declares the dependency and writes it into the record:

```vun
.deps = .{ .lib1 = .{ .path = "../lib1" } },
```

A source is a directory or the URL of an archive. For an archive `vun pkg add` fetches it to learn its hash, which the dependency then carries. `--name` and `--version` fill in what the package does not state itself.

```sh
vun pkg update
```

raises dependencies to the newest release their indexes list.

## Archives

```sh
vun pkg pack
```

writes the package's archive.

## Documentation

```sh
vun doc
```

writes the API documentation of the package. `--deps` documents its dependencies too, `--library` the toolchain's library.
