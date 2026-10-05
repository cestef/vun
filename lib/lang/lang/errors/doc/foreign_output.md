# doc.foreign_output

`vun doc` was asked to write into a directory that holds files it did not write.

The documentation is a page per namespace beside an `index.html`, a `style.css` and a
`docs.json`, and writing them replaces files of those names. A directory is the documentation's
once `vun doc` has written there, which it records with a `.vun-doc` file; a directory that is
empty or absent becomes one. Any other directory is refused whole, so that a site's own
`index.html` is never overwritten by the documentation's.

## Example

```console
$ vun doc --library -o site
error[doc.foreign_output]: `site` holds files `vun doc` did not write
  │
  ╰ help: write the documentation into an empty directory, such as `site/public/api`
```

## Fix

Name a directory that is empty, absent or written by `vun doc` before:

```console
$ vun doc --library -o site/public/api
```
