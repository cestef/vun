# manifest.invalid_field

A manifest field holds a value of the wrong shape.

Each field of the `package` literal (spec 12, section 2) has one shape: `.name` is a string, `.id`
and `.edition` are integers, `.version` and `.toolchain` are versions `major.minor.patch`,
`.include`, `.members` and `.prelude` are lists of strings, and `.deps` is a record of
dependencies. A dependency declares exactly one of `.path` and `.version`, a `.hash` is a content
hash such as `blake3:...` (spec 12, section 3), and `.kind` is `.overt` or `.foreign`.

## Example

```ov build.ov
package := .{
    .name = "blinky",
    .id = 1,
    .version = "0.1", //~ manifest.invalid_field
    .edition = 2026,
    .include = .{ "build.ov", "blinky.ov" },
};
```

## Fix

Write the value in the shape the field takes; a version has three parts:

```ov build.ov
package := .{
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .edition = 2026,
    .include = .{ "build.ov", "blinky.ov" },
};
```
