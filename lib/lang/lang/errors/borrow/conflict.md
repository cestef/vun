# borrow.conflict

A place is accessed while an exclusive borrow of it is live.

`&mut`, `&out` and `&take` borrows are exclusive: no other access to the place while they live
(spec 05, section 8). Exclusivity is checked per function with non-lexical liveness, so a borrow
lives until its last use, not to the end of its block (spec 05, section 4).

## Example

```ov
bump := (xs: &mut [4]u8) {
    first := &mut xs[0];
    xs[1] = 2; //~ borrow.conflict
    first.* = 1;
};
```

## Fix

Finish with the borrow before touching the place again:

```ov
bump := (xs: &mut [4]u8) {
    first := &mut xs[0];
    first.* = 1;
    xs[1] = 2;
};
```
