# asm.unknown_arch

A block of instructions names something that is not an architecture of `lang.arch`.

`a.asm { ... }` holds instructions of the architecture `a`, whose table is a namespace of
`lang.arch`: `lang.arch.x86_64`, `lang.arch.arm64` (spec 07, section 6). The path before `.asm`
is evaluated at compile time and must be one of them.

## Example

```vun
nothing := () {
    unsafe lang.mem.asm { //~ asm.unknown_arch
        nop
    };
};
```

## Fix

Name the architecture's namespace:

```vun
nothing := () {
    unsafe lang.arch.x86_64.asm {
        xchg rax, rax
    };
};
```
