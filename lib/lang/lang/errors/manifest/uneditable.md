# manifest.uneditable

`vun pkg add` cannot write the new dependency into the manifest.

`vun pkg add` declares a dependency by inserting its entry into `.deps` as a literal, the inverse
of the reader, keeping the manifest's layout (spec 12, section 3). The entry's name becomes a
field name, so it must be an identifier; `--name` overrides the name the package states. The
cause says what cannot be written. The manifest is left unchanged.

## Example

```console
$ vun pkg add ../hal --name hal-next
error[manifest.uneditable]: cannot declare `hal-next` in this manifest
   ╭▸ ~/app/build.vun:1:12
   │
 1 │   package := .{
   │ ┏━━━━━━━━━━━━┛
 2 │ ┃     .name = "app",
 3 │ ┃     .id = 1,
 4 │ ┃     .version = "0.1.0",
   ‡ ┃
 9 │ ┃     },
10 │ ┃ };
   │ ┗━┛
   │
   ╰ caused by: the name is not an identifier
```

## Fix

Give a name that is an identifier:

```console
$ vun pkg add ../hal --name hal_next
added hal_next at ../hal
```
