# asm.unknown_mnemonic

An instruction's mnemonic is none the architecture declares.

Each instruction of `lang.arch.<arch>` is spelled by its declaration's name, or by the name one
of its forms gives; a condition an instruction takes may follow it (`jz` on x86-64) (spec 07,
section 6). An instruction the table lacks is declared there, with its forms.

## Example

```vun
clear := () {
    unsafe lang.arch.x86_64.asm {
        xro eax, eax //~ asm.unknown_mnemonic
    };
};
```

## Fix

Spell the instruction as the table does:

```vun
clear := () {
    unsafe lang.arch.x86_64.asm {
        xor eax, eax
    };
};
```
