# `std.lints.suspicious.shadowing`

A name in a pattern always binds a new local, whatever the name means around the pattern. In a
pattern that tests a value (a `match` arm, `if p = e`, `for p = e`), a binder named as a constant
from outside the file (a name of the prelude, the package or a dependency whose value is data,
not a function, a type or a namespace) is reported: the pattern matches any value there, where
comparing with the constant was likely meant.

```vun
match parsed {
    .ok(true) => yes(),     // reported: binds a new local `true`, matching every `.ok`
    .ok(.true) => yes(),    // compares with the variant
    _ => no(),
}
```

To compare with a value, write it as anything but a plain name: a variant (`.true`), a path
(`limits.max`) or a literal. To bind, choose a name nothing outside the file uses.
