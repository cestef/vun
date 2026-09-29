# asm.unavailable

An instruction of a block needs a feature of the instruction set the target's processor lacks.

A form of an instruction may need more than every processor of its instruction set has: Thumb's
`ldrexd` and `strexd` are Arm's application profile's (`lang.arch.thumb.Feature.application`),
which Armv7-M lacks (spec 07, section 6). A target lists the features its processor has
(`lang.target.Spec.features`). Sema checks a block against the whole table, whatever the target;
a block compiled for a target of its instruction set is refused where one of its forms needs a
feature the target lacks, which would fault where it runs.

## Example

```console
$ vun build --target targets.bare.cortex_m4f pair.vun
error[asm.unavailable]: the instruction needs `application`, which the target's processor lacks
```

## Fix

Choose the instructions by the target's features at compile time, and write the others where the
processor lacks them:

```vun
load := (p: usize) -> u32 {
    $if lang.target.has(.application) {
        mut (low, high): (u32, u32) = (0, 0);
        unsafe lang.arch.thumb.asm {
            ldrexd {out(r2) low}, {out(r3) high}, [{in(r0) p}]
        };
        low
    } else {
        mut low: u32 = 0;
        unsafe lang.arch.thumb.asm {
            ldr {out(r2) low}, [{in(r0) p}]
        };
        low
    }
};
```
