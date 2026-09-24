# names.private

A name starting with `_` is used outside its scope.

A name starting with `_` is restricted: it is visible only inside its scope and the namespaces
nested within it (spec 01, section 7). The scope is the namespace that declares the name, unless
`#[visible(...)]` widens it to an enclosing namespace, the package or the workspace. The rule is the
same for declarations, struct members, files and directories, so restriction shows at every use
site. A spread cannot select a name restricted elsewhere either (spec 01, section 8).

## Example

```vun
Uart := struct {
    _raw: u32,
    new := () -> Uart { .{ ._raw = 0 } };
};

peek := (u: &Uart) -> u32 {
    u._raw //~ names.private
};
```

## Fix

Read the member from inside its namespace, and expose what outside code needs under a public name:

```vun
Uart := struct {
    _raw: u32,
    new := () -> Uart { .{ ._raw = 0 } };
    raw := (self: &Uart) -> u32 { self._raw };
};

peek := (u: &Uart) -> u32 {
    u.raw()
};
```
