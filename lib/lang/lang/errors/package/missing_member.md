# package.missing_member

A member a workspace's `build.vun` lists holds no package.

A root `build.vun` lists the packages of its workspace in `.members`, each a directory relative to
it holding a `build.vun` that declares `package` (spec 12, section 5). The members share the root's
lockfile, and a command run at the root acts on each of them. A directory that does not exist, or
whose `build.vun` declares no package, is no member.

## Example

```console
$ vun check
error[package.missing_member]: the workspace's member `pkgs/hal` holds no package
```

## Fix

Create the member's `build.vun`, or correct the path in `.members`:

```console
$ ls pkgs/hal/build.vun
pkgs/hal/build.vun
```
