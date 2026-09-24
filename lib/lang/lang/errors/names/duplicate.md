# names.duplicate

A name is declared twice in one namespace.

Each namespace declares a name once (spec 01, section 8): a file's top-level declarations, the
members of a `struct`, and the names a spread brings in all share it. A file `x.vun` and the
directory `x/` form one namespace, so a name declared in the file must not also be a file or a
directory in `x/`.

## Example

```vun
limit := 64;
limit := 128; //~ names.duplicate
```

## Fix

Give each declaration its own name, or keep one of them:

```vun
limit := 64;
max_limit := 128;
```
