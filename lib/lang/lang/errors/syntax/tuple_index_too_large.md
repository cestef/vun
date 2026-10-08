# syntax.tuple_index_too_large

A tuple position is written past what a position can hold.

A tuple field is named by its position, `t.0`, and positions are 32-bit unsigned numbers. A
larger number names no field of any tuple, so lowering stops at it.

## Example

```vun
first := (1, 2);
bad := first.4294967296; //~ syntax.tuple_index_too_large
```

## Fix

Use a position the tuple has:

```vun
first := (1, 2);
second := first.1;
```
