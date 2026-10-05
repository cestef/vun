# new.exists

`vun new` was asked to start a project in a directory that already holds one.

A project is a directory with a `build.vun` declaring `package`. The command copies a template
into the directory and never writes over a file of a project that is there.

## Example

```console
$ vun new blinky --template rp2350
error[new.exists]: `blinky` already holds a package
  help: choose a directory without a `build.vun`
```

## Fix

Name a directory that does not hold a project, or one that does not exist yet.

```console
$ vun new blinky2 --template rp2350
    Creating blinky2
```
