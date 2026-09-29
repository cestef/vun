# impl.requirement_type

An impl provides a requirement of its interface with a type the requirement does not accept.

`#[impl(I(T))]` checks its struct against the requirements of `I(T)` (spec 02, section 3): each
requirement is present, and its type is one the requirement's declared type accepts, with `Self`
and the interface's other arguments bound. Callers of the interface rely on the declared type.

## Example

```vun
Shape := interface(($Self: type) -> type { struct {
    area: (self: &Self) -> u64 = _;
}});

Square := struct { side: u32 };

#[impl(Shape(Square))] //~ impl.requirement_type
square_shape := struct {
    area := (self: &Square) -> u32 { self.side };
};
```

## Fix

Give the requirement the declared type:

```vun
Shape := interface(($Self: type) -> type { struct {
    area: (self: &Self) -> u64 = _;
}});

Square := struct { side: u32 };

#[impl(Shape(Square))]
square_shape := struct {
    area := (self: &Square) -> u64 { self.side * self.side };
};
```
