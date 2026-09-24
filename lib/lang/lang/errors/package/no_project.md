# package.no_project

A command that acts on the project runs outside any package.

`vun pkg` commands act on the project: the package of the nearest `build.vun` declaring
`package` in the current directory or above it (spec 12, section 2). A `build.vun` without
`package` is an ordinary source file and does not count.

## Example

```console
$ vun pkg pack
error[package.no_project]: no `build.vun` declaring `package` in this directory or above
```

## Fix

Run the command from the package's directory or one below it:

```console
$ cd blinky
$ vun pkg pack -o blinky-0.1.0.tar.zst
packed blinky 0.1.0 blake3:ssrso2qndj3bifvxixdmnulaam6j7xeus7zu4d6dl72gv2troica blinky-0.1.0.tar.zst
```
