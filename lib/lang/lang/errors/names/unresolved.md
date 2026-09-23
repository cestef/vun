# names.unresolved

A name does not resolve to any binding in scope.

A name is looked up in its file (its scopes, then the names spread into its enclosing namespaces,
innermost first), then among the package and its dependencies, then in the prelude (spec 01,
section 8). A spread that selects a name its namespace does not export is reported the same way.

## Example

```ov
area := (w: u32, h: u32) -> u64 {
    w * hieght //~ names.unresolved
};
```

## Fix

Spell the name as it is bound, or declare it:

```ov
area := (w: u32, h: u32) -> u64 {
    w * h
};
```
