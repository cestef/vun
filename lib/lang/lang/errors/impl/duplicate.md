# impl.duplicate

A second impl is declared for the same interface and type.

`#[impl(I(T))]` registers a witness for the pair `(I, T)`, and there is at most one impl per pair,
so every use of `I` on `T` means one thing (spec 02, section 3). An impl lives in the package of
`T` or of `I`, so the two impls are always found together.

## Example

```vun
Shape := interface(($Self: type) -> type { struct {
    area: (self: &Self) -> u64 = _;
}});

Square := struct { side: u16 };

#[impl(Shape(Square))]
square_area := struct { area := (self: &Square) -> u64 { self.side * self.side }; };

#[impl(Shape(Square))] //~ impl.duplicate
square_zero := struct { area := (self: &Square) -> u64 { 0 }; };
```

## Fix

Keep one impl; a different behavior belongs to a different type or an ordinary function:

```vun
Shape := interface(($Self: type) -> type { struct {
    area: (self: &Self) -> u64 = _;
}});

Square := struct { side: u16 };

#[impl(Shape(Square))]
square_area := struct { area := (self: &Square) -> u64 { self.side * self.side }; };
```
