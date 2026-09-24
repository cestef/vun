# layout.bits_width

The fields of a `layout.bits(uN)` struct do not add up to N bits.

`#[layout.bits(uN)]` lays a struct out bit-exact, least significant bit first, and its fields must
sum to N bits (spec 05, section 6). Each field takes the minimal bit count of its range (spec 04,
section 1, rule 7), so reserved bits are written out as `_` fields.

## Example

```vun
u2 := unsigned(2);
u28 := unsigned(28);

#[layout.bits(u32)]
Ctrl := struct { enable: bool, mode: u2, _: u28 }; //~ layout.bits_width
```

## Fix

Size the fields, here the reserved one, so that they fill the 32 bits:

```vun
u2 := unsigned(2);
u29 := unsigned(29);

#[layout.bits(u32)]
Ctrl := struct { enable: bool, mode: u2, _: u29 };
```
