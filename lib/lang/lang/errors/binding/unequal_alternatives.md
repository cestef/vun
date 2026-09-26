# binding.unequal_alternatives

The alternatives of an or-pattern bind different names, or one name with different types.

A value matching `p | q` matches one alternative, and what follows the pattern uses its names
whichever matched, so every alternative binds the same names with the same types (spec 01,
section 6).

## Example

```vun
Shape := enum { circle(u32), square(u32), empty };

size := (s: Shape) -> u32 {
    match s {
        .circle(r) | .square(side) => r, //~ binding.unequal_alternatives
        .empty => 0,
    }
};
```

## Fix

Bind the same name in every alternative:

```vun
Shape := enum { circle(u32), square(u32), empty };

size := (s: Shape) -> u32 {
    match s {
        .circle(n) | .square(n) => n,
        .empty => 0,
    }
};
```
