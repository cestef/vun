# impl.collision

A method name is declared twice for a type: by two impls a file sees, or by an impl and the type
itself.

`a.f()` looks in `T`'s namespace, then in `T`'s impls (spec 01, section 4). Impls without an
interface may come from any package, so two of them, or one and `T`'s own declaration, can give
`T` the same name; lookup would then depend on which is found first. The collision is reported
where the name is used, with every declaration it could mean.

## Example

```vun
Point := struct { x: u8 };

#[impl(Point)]
moves := struct { shift := (p: Point) -> Point { p }; };

#[impl(Point)]
nudges := struct { shift := (p: Point) -> Point { p }; };

use := (p: Point) -> Point {
    p.shift() //~ impl.collision
};
```

## Fix

Rename one method, or spell the call explicitly through the impl you mean:

```vun
Point := struct { x: u8 };

#[impl(Point)]
moves := struct { shift := (p: Point) -> Point { p }; };

#[impl(Point)]
nudges := struct { nudge := (p: Point) -> Point { p }; };

use := (p: Point) -> Point {
    p.shift()
};
```
