# package.duplicate_dep

`overt pkg add` would declare a dependency under a name the manifest already uses.

Each dependency is a binding in scope under its declared name (spec 01, section 8), so `.deps`
holds a name once. `overt pkg add` names the new dependency after the package it adds, or after
`--name` (spec 12, section 3), and leaves the manifest unchanged when the name is taken. The label
points at the existing entry.

## Example

```console
$ overt pkg add ../hal-fork
error[package.duplicate_dep]: `hal` is already a dependency
  ╭▸ ~/app/build.ov:8:16
  │
8 │         .hal = .{ .path = "../hal" },
  ╰╴               ━━━━━━━━━━━━━━━━━━━━━
```

## Fix

Declare it under another name, or remove the existing entry first to replace it:

```console
$ overt pkg add ../hal-fork --name hal_fork
added hal_fork at ../hal-fork
```
