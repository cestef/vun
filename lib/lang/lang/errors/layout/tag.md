# layout.tag

The tag type of a `layout.tag(T)` enum is not a `uN` type with a pattern for each variant.

`#[layout.tag(uN)]` stores an enum's tag as the unsigned integer `uN`, whose values number the
variants in order (spec 05, section 6). The type must be a `uN` type, and it must hold as many
values as the enum has variants.

## Example

```vun
u1 := unsigned(1);

#[layout.tag(u1)]
Mode := enum { off, on, auto }; //~ layout.tag
```

## Fix

Choose an unsigned type wide enough for every variant:

```vun
u2 := unsigned(2);

#[layout.tag(u2)]
Mode := enum { off, on, auto };
```
