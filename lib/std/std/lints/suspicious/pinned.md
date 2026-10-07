# `std.lints.suspicious.pinned`

A local bound without a type takes the type of its value, and an integer literal's type is the one
integer it writes. A `mut` local of that type can hold nothing else: `n += 1` writes a value
outside `0..=0` and traps.

```vun
mut n := 0;             // reported: its type is `Int(0..=0)`
n += 1;                 // traps
mut m: u32 = 0;         // holds every `u32`
m += 1;
```

Write the type the local holds: `mut n: u32 = 0`.
