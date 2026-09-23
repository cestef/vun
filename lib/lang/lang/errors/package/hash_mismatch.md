# package.hash_mismatch

A stored package's contents no longer hash to the hash its dependent declares.

A package is identified by the content hash of its `include` tree, and loading a stored package
recomputes that hash (spec 12, section 3). A difference means the store entry was changed after it
was fetched and verified; the notes give both hashes and name every file added, removed or
modified since the fetch, from the snapshot recorded with the entry.

## Example

A file of the stored `hal` was edited in place:

```console
$ overt check app.ov
error[package.hash_mismatch]: the stored contents of `hal` do not match its hash
   ╭▸ build.ov:8:16
   │
 8 │           .hal = .{
   │ ┏━━━━━━━━━━━━━━━━┛
 9 │ ┃             .version = "1.0.0",
10 │ ┃             .hash = "blake3:zfydsfjcithfeney4uucojbfrxp4azcrmyw3j3fdtkmwsm6cnptq",
11 │ ┃             .urls = .{ "file://hal-1.0.0.ovpkg" },
12 │ ┃         },
   │ ┗━━━━━━━━━┛
   │
   ├ note: expected `blake3:zfydsfjcithfeney4uucojbfrxp4azcrmyw3j3fdtkmwsm6cnptq`, found `blake3:sij22cdvfdz4bt4d43pgtpq7rh4gsszrkf6m53ho6yau45vraikq`
   ╰ note: `hal.ov` was modified since it was fetched
```

## Fix

Fetch the package again; an entry that differs from its snapshot is replaced by a verified copy.
Changes meant to last belong in a `.path` dependency or a new release, not in the store:

```console
$ overt pkg fetch
stored  hal blake3:zfydsfjcithfeney4uucojbfrxp4azcrmyw3j3fdtkmwsm6cnptq from file://hal-1.0.0.ovpkg
$ overt check app.ov
```
