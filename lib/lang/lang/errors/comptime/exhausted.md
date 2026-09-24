# comptime.exhausted

A compile-time call ran out of its step budget.

Evaluation is deterministic and bounded: each compile-time call may run a fixed number of steps of
its own body, so a loop that never ends is an error rather than a compiler that never ends
(spec 03, section 7). The calls it makes have their own budgets.

## Example

```vun
spin := ($n: u8) -> u8 { for { } };
Stuck := Int(0..=spin(1)); //~ comptime.exhausted
```

## Fix

Give the loop a condition that ends it:

```vun
spin := ($n: u8) -> u8 {
    mut i: u8 = 0;
    for i < n { i += 1; }
    i
};
Stuck := Int(0..=spin(1));
```
