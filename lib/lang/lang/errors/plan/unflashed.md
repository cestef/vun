# plan.unflashed

A program the plan writes to its device could not be written as the device takes it.

`b.flash(program, .uf2(.{ .family = ... }))` (spec 12, section 2) makes a `.uf2` file beside the
program it built: the bytes each loadable segment of its ELF file puts in memory, cut into
blocks at the addresses the segments are loaded from. This is reported when the program cannot
be read or is not an ELF file the toolchain reads, when a segment lies past the 32 bits of
address a block holds, and when the file cannot be written. The causes follow it.

## Example

```console
$ vun build build.vun
error[plan.unflashed]: cannot write `.vun/build/fw` as a UF2 file
  caused by: a segment of the program lies past what a block addresses
  caused by: address 0x100000000 does not fit the 32 bits a block holds
```

## Fix

Place the program's sections in the memory of a device of 32-bit addresses (`memory` and
`sections` of the program's options), or flash a device whose method takes larger addresses.
