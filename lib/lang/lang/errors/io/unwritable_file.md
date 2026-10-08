# io.unwritable_file

A file the command writes cannot be written.

Commands write the manifest (`vun pkg add`), the lockfile (the commands that compile, `vun pkg add` and `update`), archives (`vun
pkg pack`) and the store (`vun util cache`, spec 12, section 3). The file system's reason is kept as
the chain of causes (spec 10, section 2).

## Example

```console
$ vun pkg pack -o release/blinky-0.1.0.tar.zst
error[io.unwritable_file]: cannot write `release/blinky-0.1.0.tar.zst`
  │
  ╰ caused by: Permission denied (os error 13)
```

## Fix

Write where the permissions allow it, or grant them; missing directories are created:

```console
$ vun pkg pack -o dist/blinky-0.1.0.tar.zst
packed blinky 0.1.0 blake3:ssrso2qndj3bifvxixdmnulaam6j7xeus7zu4d6dl72gv2troica dist/blinky-0.1.0.tar.zst
```
