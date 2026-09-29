# types.no_impl

An operator is used on a type that has no impl of its interface.

Every operator desugars to a function of an interface in `lang.ops` (spec 02, section 4): `a + b`
calls `add` of `lang.ops.Add`, as the impl for the type of `a` gives it, and `a == b` calls `eq`
of `lang.ops.Eq`. Integers, floats, slices and the types of `lang` and `std` have their impls; a
type of your own has those you write, or derive, as `std.cmp.derive_eq` does for `Eq`.

## Example

```vun
Point := struct { x: u8, y: u8 };

far := (a: Point, b: Point) -> Point {
    a + b //~ types.no_impl
};
```

## Fix

Give the type an impl of the interface, whose function the operator calls:

```vun
Point := struct { x: u8, y: u8 };

#[impl(lang.ops.Add(Point))]
point_add := struct {
    Output := ($R: type) -> type { Point };
    add := (a: Point, b: Point) -> Point { Point{ .x = (a.x + b.x).wrap(u8), .y = (a.y + b.y).wrap(u8) } };
};

far := (a: Point, b: Point) -> Point { a + b };
```
