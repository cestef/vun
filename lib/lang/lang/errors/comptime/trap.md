# comptime.trap

A trap fired while evaluating code at compile time.

Compile time and run time share one semantics (spec 00, guarantee 4), so the traps of run time fire
at compile time too: a division by zero, a shift past any width, a `.to` or compound assignment
whose value falls outside its type (spec 04). The error points at the value being computed, labels
where the trap fired, and lists each call it fired in (spec 03, section 7).

## Example

```vun
half := ($n: u8) -> u8 { 10 / n };
Small := Int(0..=half(0)); //~ comptime.trap
```

## Fix

Call the function with a value it can compute, or make the divisor's type exclude zero, so the
division is no trap site:

```vun
half := ($n: Int(1..=255)) -> u8 { 10 / n };
Small := Int(0..=half(2));
```
