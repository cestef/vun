# asm.immediate_too_wide

An integer operand does not fit the immediate of any form that could take it.

An immediate is encoded in as many bits as its form gives, sign- or zero-extended to the width
of the operation (spec 07, section 6); a value the operation would read differently is an error
rather than a silently different constant.

## Example

```vun
big := () {
    unsafe lang.arch.x86_64.asm {
        add eax, 5000000000 //~ asm.immediate_too_wide
    };
};
```

## Fix

Load the constant into a register at a width that holds it, then use the register:

```vun
big := () {
    unsafe lang.arch.x86_64.asm {
        mov rcx, 5000000000
        add rax, rcx
    };
};
```
