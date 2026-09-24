# package.stale_lock

The project's lockfile, `lock.ov`, does not record the versions its graph selects.

`lock.ov` records every version the dependency graph requires and the hash it was verified against
(spec 12, section 4), one entry per line. It is written by `overt pkg fetch` and `overt pkg add`,
never by hand; a lockfile that records something else, after a manifest was edited or a merge, is
reported when the selection is first needed. While a package of the graph is absent, the lockfile
is not judged: dependencies resolve through it instead (spec 12, section 4).

## Example

`build.ov` no longer declares `hal`, which the lockfile still records:

```console
$ cat lock.ov
/// Written by `overt pkg` from the selected versions; do not edit.
version := 1;
deps := .{
    .{ .name = "hal", .version = "1.0.0", .id = .{ .id = 2, .major = 1 }, .hash = "blake3:zfyd..." },
};
$ overt check app.ov
error[package.stale_lock]: `lock.ov` does not record the selected versions
  ╭▸ lock.ov:4:5
  │
4 │     .{ .name = "hal", .version = "1.0.0", .id = .{ .id = 2, .major = 1 }, .hash = "blake3:zfyd..." },
  │     ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  │
  ╰ help: rewrite it with `overt pkg fetch`
```

The primary label is the first entry the selection does not have; the others are labeled too, and
each entry the selection needs but the lockfile lacks is a note.

## Fix

Write the lockfile again from the manifests. Fetching walks the graph as declared, not through the
lockfile, so it rewrites a stale one:

```console
$ overt pkg fetch
$ cat lock.ov
/// Written by `overt pkg` from the selected versions; do not edit.
version := 1;
deps := .{};
$ overt check app.ov
```
