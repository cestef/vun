# asm.unknown_mnemonic

A statement of a block calls something that is no instruction of the block's architecture.

Each statement of a block calls an instruction `lang.arch.<arch>` declares, or makes or marks a
label (spec 07, section 6). A function, or anything else callable, runs outside the block: call
it before or after it, or reach it by its address with the architecture's call instruction.

## Example

```vun
helper := () {};

clear := () {
    unsafe lang.arch.asm({
        ..lang.arch.x86_64.names;
        xor(eax, eax);
        helper(); //~ asm.unknown_mnemonic
    });
};
```

## Fix

Call the function outside the block:

```vun
helper := () {};

clear := () {
    unsafe lang.arch.asm({
        ..lang.arch.x86_64.names;
        xor(eax, eax);
    });
    helper();
};
```
