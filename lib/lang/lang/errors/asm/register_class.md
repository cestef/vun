# asm.register_class

A register is of another class than the operand position takes: a general register where an
SSE or floating-point one is expected, or the reverse.

Each operand position of a form takes registers of one class (spec 07, section 6). A value bound
in braces is held in a register of the class its type needs: an integer or an address in a
general one, a float in a floating-point one.

## Example

```vun
widen := () {
    unsafe lang.arch.x86_64.asm {
        cvtss2sd rax, xmm1 //~ asm.register_class
    };
};
```

## Fix

Name a register of the class the position takes:

```vun
widen := () {
    unsafe lang.arch.x86_64.asm {
        cvtss2sd xmm0, xmm1
    };
};
```
