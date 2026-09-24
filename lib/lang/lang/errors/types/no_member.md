# types.no_member

A value's type has no member of the name used after `.`.

`a.f` looks in the namespace of `a`'s type, then in its impls (spec 01, section 4). Inside a
generic body, a value whose type is constrained by an interface may use only that interface's
members; the check runs per instantiation and is reported against the generic definition (spec 02,
section 3).

## Example

```ov
Shape := interface(($Self: type) -> type { struct {
    area: (self: &Self) -> u64 = _;
}});

describe := ($S: Shape, s: &S) -> u64 {
    s.area() + s.perimeter() //~ types.no_member
};
```

## Fix

Use only the members the interface declares, or declare the member it lacks:

```ov
Shape := interface(($Self: type) -> type { struct {
    area: (self: &Self) -> u64 = _;
    perimeter: (self: &Self) -> u64 = _;
}});

describe := ($S: Shape, s: &S) -> u128 {
    s.area() + s.perimeter()
};
```
