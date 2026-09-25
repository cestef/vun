# link.export_mismatch

An export provides a typed link symbol with a value of another type.

A declaration the linker provides, such as `std.task.sleep` or `std.sync.enter`, may be satisfied
by a `#[link.export]` of the same symbol in the same program. When both sides are Vun, the
export's value must be accepted where the symbol's declared type is expected: the same parameters
and result, and every marker the declaration promises, such as `#[panic.never]` (spec 08,
section 1, rule 3). Otherwise the program would call the export with arguments it does not take.

## Example

```vun
Runtime := struct {
    #[link.export(std.task.symbols.sleep)] //~ link.export_mismatch
    sleep := (ticks: u32) { };
};
```

## Fix

Give the export the declared type, here `() -> ()`:

```vun
Runtime := struct {
    #[link.export(std.task.symbols.sleep)]
    sleep := () { };
};
```
