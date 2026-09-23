# package.stale_lock

The project's `overt.lock` does not record the versions its graph selects.

`overt.lock` records every version the dependency graph requires and the hash it was verified
against (spec 12, section 4). It is written by `overt pkg fetch` and `overt pkg add`, never by
hand; a lockfile that records something else, after a manifest was edited or a merge, is reported
when the selection is first needed.

## Example

`build.ov` no longer declares `hal`, which the lockfile still records:

```console
$ cat overt.lock
# overt.lock 1 (written by `overt pkg`; do not edit)
dep hal 1.0.0 0000000000000002/1 blake3:zfydsfjcithfeney4uucojbfrxp4azcrmyw3j3fdtkmwsm6cnptq
$ overt check app.ov
error[package.stale_lock]: `overt.lock` does not record the selected versions
  │
  ╰ help: rewrite it with `overt pkg fetch`
```

## Fix

Write the lockfile again from the manifests:

```console
$ rm overt.lock
$ overt pkg fetch
$ cat overt.lock
# overt.lock 1 (written by `overt pkg`; do not edit)
$ overt check app.ov
```
