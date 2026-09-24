# place.ambiguous_join

A place is used after paths that leave it in different states join.

Place states are checked on the control-flow graph: a place that is `init` on one incoming path
and `uninit` on another cannot be used after the join (spec 05, section 2). No run-time flag
records which path was taken.

The same holds for `defer` (spec 05, section 3): a deferred statement consuming a linear value runs
on the exits where the value is still held and is skipped where it was moved, decided when
compiling. At an exit reached with the value moved on some paths and held on others, neither
choice is right, and this error names the value.

## Example

```vun
pick := (flag: bool) -> u8 {
    mut x: u8;
    if flag { x = 1; }
    x //~ place.ambiguous_join
};
```

## Fix

Give the place the same state on every path:

```vun
pick := (flag: bool) -> u8 {
    mut x: u8;
    if flag { x = 1; } else { x = 0; }
    x
};
```
