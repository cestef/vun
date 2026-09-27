# asm.unknown_operand

An operand is none the architecture has: a word that names no register or condition, a label
the block does not mark, or an expression that is not an operand.

An operand is a register or a condition as the table names it, an integer, a label `'name` the
block marks with `'name:`, memory `[...]`, or a Vun value bound to it in braces (spec 07,
section 6). Other Vun expressions go in braces: `{in f(x)}`.

## Example

```vun
two := () -> u64 {
    mut r: u64 = 0;
    unsafe lang.arch.x86_64.asm {
        mov {out r}, rxa //~ asm.unknown_operand
    };
    r
};
```

## Fix

Name a register of the table:

```vun
two := () -> u64 {
    mut r: u64 = 0;
    unsafe lang.arch.x86_64.asm {
        mov {out r}, rax
    };
    r
};
```
