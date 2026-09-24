# place.move_out_of_borrow

A linear value is moved out of a place reached through a borrow or an owning pointer.

Reading a whole linear value moves it (spec 05, section 1), and a borrow does not own what it
points to: moving the value out would leave the lender's place empty behind its back. Match or
read through the borrow itself instead, which binds or reads by borrow (spec 01, section 4).

## Example

```vun
File := #[kind.linear] struct { fd: u32 };

fd := (slot: &?File) -> u32 {
    match slot.* { //~ place.move_out_of_borrow
        .some(f) => f.fd,
        .none => 0,
    }
};
```

## Fix

Match on the borrow, so that `f` is a `&File`:

```vun
File := #[kind.linear] struct { fd: u32 };

fd := (slot: &?File) -> u32 {
    match slot {
        .some(f) => f.fd,
        .none => 0,
    }
};
```
