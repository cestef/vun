# names.private

A name starting with `_` is used outside the namespace that declares it.

A name starting with `_` is visible only inside the namespace that declares it and the namespaces
nested within it (spec 01, section 7). The rule is the same for declarations, struct members,
files and directories, so privacy shows at every use site. A spread cannot select a private name
either (spec 01, section 8).

## Example

```ov
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

```ov
Uart := struct {
    _raw: u32,
    new := () -> Uart { .{ ._raw = 0 } };
    raw := (self: &Uart) -> u32 { self._raw };
};

peek := (u: &Uart) -> u32 {
    u.raw()
};
```
