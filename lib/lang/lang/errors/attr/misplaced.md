# attr.misplaced

An attribute is written where it does not apply.

An attribute whose type is `lang.Attribute(on)` applies only at the places `on` lists
(`lang.Place`: declarations, functions, types, fields, parameters); written elsewhere, nothing
would read it, so it is reported rather than ignored (spec 02, section 5). Every attribute of
`lang` declares its places, and a library attribute may too; an attribute of any other type is
plain data, written anywhere.

## Example

```vun
#[layout.c] //~ attr.misplaced
area := (w: u32, h: u32) -> u64 { w * h };
```

## Fix

`layout.c` lays out a type; write it on the `struct` it is meant for:

```vun
#[layout.c]
Size := struct { w: u32, h: u32 };

area := (s: Size) -> u64 { s.w * s.h };
```
