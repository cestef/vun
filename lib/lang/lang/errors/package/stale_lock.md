# package.stale_lock

The project's `overt.lock` does not record the versions its graph selects.

`overt.lock` records every version the dependency graph requires and the hash it was verified
against (spec 12, section 4). It is written by `overt pkg fetch` and `overt pkg add`, never by
hand; a lockfile that records something else, after a manifest was edited or a merge, is reported
when the selection is first needed. While a package of the graph is absent, the lockfile is not
judged: dependencies resolve through it instead (spec 12, section 4).

## Example

`build.ov` no longer declares `hal`, which the lockfile still records:

```console
$ cat overt.lock
# overt.lock 1 (written by `overt pkg`; do not edit)
dep hal 1.0.0 0000000000000002/1 blake3:zfydsfjcithfeney4uucojbfrxp4azcrmyw3j3fdtkmwsm6cnptq
$ overt check app.ov
error[package.stale_lock]: `overt.lock` does not record the selected versions
  ╭▸ overt.lock:2:1
  │
2 │ dep hal 1.0.0 0000000000000002/1 blake3:zfydsfjcithfeney4uucojbfrxp4azcrmyw3j3fdtkmwsm6cnptq
  │ ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  │
  ╰ help: rewrite it with `overt pkg fetch`
```

The primary label is the first line the selection does not have; the others are labeled too, and
each line the selection needs but the lockfile lacks is a note.

## Fix

Write the lockfile again from the manifests. Fetching walks the graph as declared, not through the
lockfile, so it rewrites a stale one:

```console
$ overt pkg fetch
$ cat overt.lock
# overt.lock 1 (written by `overt pkg`; do not edit)
$ overt check app.ov
```
