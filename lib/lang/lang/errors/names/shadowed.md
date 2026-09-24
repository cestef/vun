# names.shadowed

A local binding reuses a name already visible where it is declared.

There is no shadowing within a file (spec 01, section 3): a local binding (in a block, a parameter,
a pattern) may not reuse the name of an enclosing local or parameter, or of a member of an
enclosing namespace. Names from the prelude or a dependency may be reused, so adding a name to a
library never breaks its users.

## Example

```vun
total := (xs: &[u32]) -> u64 {
    sum: u64 = 0;
    for x in xs {
        sum := x; //~ names.shadowed
    }
    sum
};
```

## Fix

Assign to the existing binding, made `mut`, or give the new one its own name:

```vun
total := (xs: &[u32]) -> u64 {
    mut sum: u64 = 0;
    for x in xs {
        sum += x;
    }
    sum
};
```
