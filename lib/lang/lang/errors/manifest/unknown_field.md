# manifest.unknown_field

A manifest record has a field that its reader does not know.

The `package` literal of `build.ov` (spec 12, section 2) and each entry of its `.deps` are read
field by field, without evaluating anything, so a misspelled field would otherwise be ignored in
silence. `package` knows `.name`, `.id`, `.version`, `.edition`, `.toolchain`, `.include`,
`.deps`, `.members` and `.prelude`; a dependency knows `.version`, `.hash`, `.urls`, `.path` and
`.kind`. The help names the closest known field.

## Example

```ov build.ov
package := .{
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .edition = 2026,
    .include = .{ "build.ov", "blinky.ov" },
    .prelde = .{ "std.prelude" }, //~ manifest.unknown_field "did you mean `.prelude`?"
};
```

## Fix

Spell the field as the reader knows it:

```ov build.ov
package := .{
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .edition = 2026,
    .include = .{ "build.ov", "blinky.ov" },
    .prelude = .{ "std.prelude" },
};
```
