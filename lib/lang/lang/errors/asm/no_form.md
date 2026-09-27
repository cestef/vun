# asm.no_form

No form of the instruction takes its operands.

Each instruction declares its forms: the kinds of operands each takes, in order, and the widths
it is defined at (spec 07, section 6). The operands are checked against each form in turn, and
the first that takes them encodes the instruction.

## Example

```vun
store := () {
    unsafe lang.arch.x86_64.asm {
        mov 1, rax //~ asm.no_form
    };
};
```

## Fix

Write the operands in the order a form takes them, the destination first on x86-64:

```vun
store := () {
    unsafe lang.arch.x86_64.asm {
        mov rax, 1
    };
};
```
