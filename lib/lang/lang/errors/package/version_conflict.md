# package.version_conflict

Two packages of the graph require one version of a package with different contents.

A package is identified by its id and major version, and a version by the content hash of its
tree (spec 12, section 3). Two dependents may require different versions, and the largest is
selected, but one version has one hash: two hashes for it mean that one of them does not hold the
release they both name (spec 12, section 4). The notes give each hash and who requires it.

## Example

```console
$ overt pkg fetch
present hal blake3:zfydsfjcithfeney4uucojbfrxp4azcrmyw3j3fdtkmwsm6cnptq
present hal blake3:i2awm7xqhnlzsujcu5ukmn2owcntcqyycpr2d4kuhqquxzgxkz6q
error[package.version_conflict]: `hal` `1.0.0` is required with two hashes
  │
  ├ note: `blake3:zfydsfjcithfeney4uucojbfrxp4azcrmyw3j3fdtkmwsm6cnptq` required by `the project`
  ╰ note: `blake3:i2awm7xqhnlzsujcu5ukmn2owcntcqyycpr2d4kuhqquxzgxkz6q` required by `drv`
```

## Fix

Find which hash is the release and declare it in the other dependent's `.hash`, here the
project's:

```console
$ grep hash build.ov
            .hash = "blake3:i2awm7xqhnlzsujcu5ukmn2owcntcqyycpr2d4kuhqquxzgxkz6q",
$ overt pkg fetch
present hal blake3:i2awm7xqhnlzsujcu5ukmn2owcntcqyycpr2d4kuhqquxzgxkz6q
```
