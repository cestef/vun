# panic.never_violated

A `#[panic.never]` function may trap.

`#[panic.never]` is checked transitively: the function's body may contain no undischarged trap
site, and it may call only `#[panic.never]` functions (spec 07, section 3). Trap sites are a failed
`.to`, a compound assignment out of range, a zero divisor, an unproven index, unwrapping `.none`
and `std.panic.fail`; `vun check --traps` lists those that remain.

## Example

```vun
halve := (x: u32) -> u32 { x / 2 };

#[panic.never]
tick := (x: u32) -> u32 {
    halve(x) //~ panic.never_violated
};
```

## Fix

Mark the callee `#[panic.never]` too; its body has no trap site, since the divisor excludes 0:

```vun
#[panic.never]
halve := (x: u32) -> u32 { x / 2 };

#[panic.never]
tick := (x: u32) -> u32 {
    halve(x)
};
```
