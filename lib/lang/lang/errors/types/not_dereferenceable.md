# types.not_dereferenceable

`.*` reads through a value that is not a borrow, a pointer or a type with a `deref`.

`p.*` is the value `p` borrows or points to (spec 05, section 1), or the one a type reads through
by its `deref` member. A value of any other type, such as an integer that is not behind a
borrow, has nothing to read through.

## Example

```vun
double := (n: u8) -> u16 {
    n.* * 2 //~ types.not_dereferenceable
};
```

## Fix

Use the value itself, or borrow it where the borrow is meant:

```vun
double := (n: u8) -> u16 {
    n * 2
};
```
