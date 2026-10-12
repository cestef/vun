# tool.split_place

`vun util ffi bind --split` was given no place inside a package to write the files to.

The file for each directory of headers names the types of the others by their path from the
package (spec 01, section 8), which a file outside any package has none of. Write the bindings
with `--out` to a file inside a package.

## Example

```console
$ vun util ffi bind gtk.h --split
error[tool.split_place]: `--split` needs `--out` to name a file inside a package
```

## Fix

Name the file the bindings are spread from, in the package's directories:

```console
$ vun util ffi bind gtk.h --split --out app/gtk/sys.vun
```

The files for the directories are written beside it, in `app/gtk/sys/`.
