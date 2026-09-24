# package.stale_lock

The project's lockfile, `lock.vun`, does not record the versions its graph selects.

`lock.vun` records every version the dependency graph requires and the hash it was verified against
(spec 12, section 4), one entry per line. It is written by `vun pkg fetch` and `vun pkg add`,
never by hand; a lockfile that records something else, after a manifest was edited or a merge, is
reported when the selection is first needed. While a package of the graph is absent, the lockfile
is not judged: dependencies resolve through it instead (spec 12, section 4).

## Example

`build.vun` no longer declares `hal`, which the lockfile still records:

```console
$ cat lock.vun
/// Written by `vun pkg` from the selected versions; do not edit.
version := 1;
deps := .{
    .{ .name = "hal", .version = "1.0.0", .id = .{ .id = 2, .major = 1 }, .hash = "blake3:zfyd..." },
};
$ vun check app.vun
error[package.stale_lock]: `lock.vun` does not record the selected versions
  ╭▸ lock.vun:4:5
  │
4 │     .{ .name = "hal", .version = "1.0.0", .id = .{ .id = 2, .major = 1 }, .hash = "blake3:zfyd..." },
  │     ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  │
  ╰ help: rewrite it with `vun pkg fetch`
```

The primary label is the first entry the selection does not have; the others are labeled too, and
each entry the selection needs but the lockfile lacks is a note.

## Fix

Write the lockfile again from the manifests. Fetching walks the graph as declared, not through the
lockfile, so it rewrites a stale one:

```console
$ vun pkg fetch
$ cat lock.vun
/// Written by `vun pkg` from the selected versions; do not edit.
version := 1;
deps := .{};
$ vun check app.vun
```
