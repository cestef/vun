# manifest.unknown_field

A manifest record has a field that its reader does not know.

The `package` literal of `build.vun` (spec 12, section 2) and each entry of its `.deps` are read
field by field, without evaluating anything, so a misspelled field would otherwise be ignored in
silence. `package` knows `.name`, `.id`, `.version`, `.edition`, `.toolchain`, `.include`,
`.deps`, `.members` and `.prelude`; a dependency knows `.version`, `.hash`, `.urls`, `.path` and
`.kind`. The help names the closest known field.

## Example

```vun build.vun
package := .{
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .edition = 2026,
    .include = .{ "build.vun", "blinky.vun" },
    .prelde = .{ "std.prelude" }, //~ manifest.unknown_field "did you mean `.prelude`?"
};

build := (b: &mut std.build.Build) {};
```

## Fix

Spell the field as the reader knows it:

```vun build.vun
package := .{
    .name = "blinky",
    .id = 1,
    .version = "0.1.0",
    .edition = 2026,
    .include = .{ "build.vun", "blinky.vun" },
    .prelude = .{ "std.prelude" },
};

build := (b: &mut std.build.Build) {};
```
