# borrow.escape

A borrow outlives the scope it was taken in.

A scoped value (a borrow, or anything containing one) may be returned only from a function taking
at least one borrow, and it is then tied to all of that call's borrowed arguments (spec 05,
section 4). A borrow of a local, or one tied to a critical section (spec 07, section 2), cannot
leave its scope.

## Example

```vun
dangling := () -> &u8 {
    x: u8 = 1;
    &x //~ borrow.escape
};
```

## Fix

Return the value itself, or a borrow of something the caller lends:

```vun
value := () -> u8 {
    x: u8 = 1;
    x
};
```
