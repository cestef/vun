# manifest.missing_package

A file read as a manifest does not declare `package`.

A directory holds a package when its `build.vun` declares `package` (spec 12, section 2); any
other `build.vun` is an ordinary source file, so checking it reports nothing. A command that reads
a given file as a manifest, such as `vun debug dump manifest`, needs the declaration under that exact
name.

## Example

```console
$ cat build.vun
pkg := .{
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .edition = 2026,
    .include = .{ "build.vun", "blinky.vun" },
};
$ vun debug dump manifest build.vun
error[manifest.missing_package]: no `package` declaration
  ╭▸ build.vun:1:1
  │
1 │ pkg := .{
  │ ━
  │
  ╰ help: declare `package := .{ .name = "...", ... };`
```

## Fix

Declare the manifest as `package`:

```console
$ cat build.vun
package := .{
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .edition = 2026,
    .include = .{ "build.vun", "blinky.vun" },
};
$ vun debug dump manifest build.vun
Manifest {
    name: "blinky",
    id: 1,
    ...
}
```
