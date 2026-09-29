# asm.unknown_arch

A block of instructions calls instructions of more than one architecture.

A block is written in one instruction set: its instructions are declarations of one namespace of
`lang.arch`, such as `lang.arch.x86_64` or `lang.arch.arm64`, whose table sema reads the block
against (spec 07, section 6). The first instruction a block calls gives its architecture.

## Example

```vun
back := () {
    unsafe lang.arch.asm({
        ..lang.arch.x86_64.names;
        ret();
        lang.arch.arm64.control.ret(); //~ asm.unknown_arch
    });
};
```

## Fix

Write a block per architecture, each where the target's instruction set is its own:

```vun
back := () {
    $if lang.target.current.arch == .x86_64 {
        unsafe lang.arch.asm({ ..lang.arch.x86_64.names; ret(); });
    } else {
        unsafe lang.arch.asm({ ..lang.arch.arm64.names; ret(); });
    }
};
```
