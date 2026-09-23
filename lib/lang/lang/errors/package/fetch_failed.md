# package.fetch_failed

No mirror of an archive dependency served it.

`overt pkg fetch` tries the `.urls` of each archive dependency the store lacks in order, and
stores the first archive whose contents hash to the declared `.hash`; `overt pkg add` fetches an
archive once to learn its hash (spec 12, section 3). When every mirror fails, each is listed with
its chain of causes (spec 10, section 2): a download that failed, an archive that does not unpack,
contents that hash to something else, or a store that cannot be written.

## Example

```console
$ overt pkg fetch
error[package.fetch_failed]: cannot fetch `hal` (`blake3:i2awm7xqhnlzsujcu5ukmn2owcntcqyycpr2d4kuhqquxzgxkz6q`)
   ╭▸ ~/app/build.ov:8:16
   │
 8 │           .hal = .{
   │ ┏━━━━━━━━━━━━━━━━┛
 9 │ ┃             .version = "1.0.0",
10 │ ┃             .hash = "blake3:i2awm7xqhnlzsujcu5ukmn2owcntcqyycpr2d4kuhqquxzgxkz6q",
11 │ ┃             .urls = .{ "https://example.invalid/hal-1.0.0.ovpkg" },
12 │ ┃         },
   │ ┗━━━━━━━━━┛
   │
   ├ note: from `https://example.invalid/hal-1.0.0.ovpkg`
   ├ caused by: cannot download the archive
   ╰ caused by: io: failed to lookup address information: Name or service not known
```

## Fix

Declare a mirror that serves the same bytes; any mirror is as valid as another, since the hash is
checked:

```console
$ grep urls build.ov
            .urls = .{ "https://example.invalid/hal-1.0.0.ovpkg", "file://hal-1.0.0.ovpkg" },
$ overt pkg fetch
stored  hal blake3:i2awm7xqhnlzsujcu5ukmn2owcntcqyycpr2d4kuhqquxzgxkz6q from file://hal-1.0.0.ovpkg
```
