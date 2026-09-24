# syntax.unknown_borrow

The words after `&` name no borrow mode.

A borrow mode is one of the rows of the table `borrows` (appendix B): `&T`, `&mut T`, `&out T`,
`&take T`, `&temp T`, `&temp mut T` and `&static T`. Each is named by the transition it performs
on the place it borrows (spec 05, section 2); words that name no row, such as `temp take`, have no
meaning.

## Example

```vun
consume := (a: &temp take u8) {}; //~ syntax.unknown_borrow
```

## Fix

Write one of the modes. A value consumed in place is taken by `&take`:

```vun
consume := (a: &take u8) {};
```
