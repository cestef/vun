# asm.unknown_operand

An operand is none the architecture has: a value that names no register, condition or choice, a
run-time value not bound to a register, or an output that is no place.

An operand is a register, a condition or a choice as the table names it, an integer known at
compile time, a label, memory (`mem(...)`), or a Vun value bound to a register by `in`, `out` or
`inout` (spec 07, section 6). A value known only at run time is passed with `in(e)`.

## Example

```vun
two := (x: u64) -> u64 {
    mut r: u64 = 0;
    unsafe lang.arch.asm({
        ..lang.arch.x86_64.names;
        mov(out(r), x); //~ asm.unknown_operand
    });
    r
};
```

## Fix

Bind the value to a register:

```vun
two := (x: u64) -> u64 {
    mut r: u64 = 0;
    unsafe lang.arch.asm({
        ..lang.arch.x86_64.names;
        mov(out(r), in(x));
    });
    r
};
```
