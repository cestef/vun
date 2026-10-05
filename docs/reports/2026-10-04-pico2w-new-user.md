# A new user on a Pico 2 W (2026-10-04, second pass)

A session playing someone who has cloned the repository and has a Raspberry Pi Pico 2 W on USB (in
BOOTSEL), and wants a program running on it. Checkout at `71f698d8` on x86_64 Fedora 43. It follows
[2026-10-04-pico2w.md](2026-10-04-pico2w.md), which brought the chip up; this one starts from the
board package that report produced and asks what a newcomer meets.

Verdict: **it works, and the compiler side is pleasant** (builds take 0.2 s, diagnostics are
good, a 644-byte release image). What fails the newcomer is everything around the compiler: no
example, no way to start a project, no flash command, no way to see what a program did, and a
halted board that only a button press recovers.

## Setup that was not the user's flow

- `~/.local/bin/vun` was the toolchain installed at 12:28, before the Cortex-M33 target existed.
  Instead of `cargo xtask install` on the laptop, `vun-cli` was built `--release` on `petal.wg`,
  copied back, and run with `VUN_LIBRARY=<checkout>/lib`. `vun --version` then printed `vun 0.1.0`
  with no commit, because the mirror has no `.jj`.
- `sudo dnf install arm-none-eabi-gcc-cs arm-none-eabi-binutils-cs` was run on this machine (1.4 GiB).

## What ran

`hello-pico/` (a `build.vun` and one `app.vun`, kept outside the repository):

```vun
package := .{
    .name = "hello_pico", .id = 0x0000_0000_0000_1234, .version = "0.1.0", .edition = 2026,
    .target = "bare.thumbv8m_main.eabihf",
    .include = .{ "build.vun", "app.vun" },
    .deps = .{ .rp2350 = .{ .path = "<checkout>/lib/boards/rp2350" } },
};

build := (b: &mut std.build.Build) {
    board := b.dep(.rp2350, .{});
    fw := b.program(.{
        .root = "app.vun",
        .target = targets.bare.thumbv8m_main.eabihf,
        .deps = &[.{ .name = .rp2350, .dep = board }],
        .memory = rp2350.layout.memory,
        .sections = rp2350.layout.sections,
    });
    b.flash(fw, .uf2(.{ .family = 0xe48bff59 }));
};
```

Each program below was built with `vun build` (debug and, for the first, `--release`), loaded, and
judged by what the board did next. With no console, the only signal is the USB bus, so every
program ends by rebooting to BOOTSEL and **hiding the mass storage interface** on success
(`rp2350.rom.bootsel_hiding(rp2350.rom.interface_mass_storage)`); a bare `bootsel()` looks the same
as a program that never started.

| Program | Result |
|---------|--------|
| GPIO 15 high, `f32` multiply, reboot hiding the drive | ran: new device number, no drive. UF2 copied to the mounted drive, and `picotool load -x` of the UF2 and of the ELF |
| same, `--release` | ran (`text` 644 bytes, UF2 1536 bytes) |
| runtime out-of-bounds index, `.trap = rp2350.halt` | board gone from the bus (halted). Recovery needed BOOTSEL by hand |
| same, `.trap = on_trap` rebooting to BOOTSEL hiding the drive | ran: drive hidden, so the trap handler ran |
| read from `0x6000_0000` | bus fault: board back in BOOTSEL with the drive visible (`rp2350.fault.record` path) |
| SysTick at 150 000 reload, handler counts a `mut` static, `main` waits for 100 | ran: drive hidden, so the interrupt fired and the static was read back |

Not shown by these: that the float result was computed at run time (the compiler may have folded
it), the level of GP15 (no LED on the Pico 2 W's GPIOs), the UART (no adapter), the Hazard3 cores.

## Rough edges, worst first

1. **No way to see anything.** No console, the on-board LED is on the CYW43 radio (no driver), and
   the UART needs an adapter and has an open loopback bug (`c810d7b8`). A newcomer's "hello" is
   "the drive disappeared". The README promises firmware as a use; the first program should be
   able to show life. Candidates: a CYW43 LED driver, a USB CDC console on the RP2350's USB, or at
   least a documented board test that uses `bootsel_hiding` as above.
2. **A panic with the board's own `halt` is a dead end.** `rp2350.halt` waits for interrupts
   forever: no USB, no BOOTSEL, so the board must be unplugged with the button held. A fault, by
   contrast, reboots into BOOTSEL with a record. The two should agree, and the default trap for a
   console-less board should be the one that gives the device back. The `Config.trap` field made
   writing one easy (a plain function), which was the good part.
3. **The fault record cannot be read with stock tools.** It lives in the watchdog scratch
   registers; `picotool save -r 0x400d800c 0x400d801c` fails with `Save range crosses unmapped
   memory`. Reading it needs another Vun program decoding bits through the USB interfaces
   (`docs/dev/pitfalls.md`). A reader that copies the record somewhere `picotool save` can read
   (flash, or an SRAM range that survives) would make the feature usable.
4. **No `vun flash`, and `b.flash` does nothing on `vun build`.** The plan records the step and the
   UF2 is written beside the ELF, but no command acts on it. The flow was: `udisksctl mount -b
   /dev/sda1`, `cp app.uf2 /run/media/$USER/RP2350/`, or `picotool load -x -t elf .vun/build/app`
   (`-t elf` because the output has no extension). With the drive hidden by the previous program,
   only the `picotool` route works, so a user needs both.
5. **`vun run` on a firmware package runs it under qemu and segfaults.** `vun run` printed
   `Running .vun/build/app` and `qemu: uncaught target signal 11`. `vun target list` says the
   target runs `elsewhere`; `run` should refuse with a message that says to flash, or run the flash
   step.
6. **There is no project starter and no example.** No `vun new`/`vun init` (`vun --help` has none),
   no `corpus` program for `rp2350` (only `bare_metal.vun` for another board), and no README in
   `lib/boards/rp2350`. The `build.vun` above was assembled from `corpus/accept/build/build.vun`,
   the stm32f4 shape and `runtime.vun`. A package `.id` is a hand-written 64-bit number with no
   word on how to pick it, and the board dependency is an absolute path.
7. **Missing `arm-none-eabi-gcc` is reported as an OS error.** ``backend.tool: `arm-none-eabi-gcc`
   could not compile `program.c` / it could not be started / No such file or directory (os error
   2)``. `vun target list` does say `lacks arm-none-eabi-gcc`, but the build error neither names the
   package nor points there. `--backend native` helps nothing: it needs the same tool as its
   linker, and `--linker lld` fails with `link.unsupported: linker lld does not link objects of
   format elf`, so no bare-metal Arm link exists without the GNU toolchain.
8. **A stale toolchain looks like a user error.** The 12:28 binary against the new library says
   ``unknown field `.target` in `package` `` on the board's own `build.vun`. The cause is version
   skew between binary and library; `vun check` could compare the toolchain's version with the
   library's and say so.
9. **The RP2350 board is thin.** There is no timer or delay module (the stm32f4 board has `time`
   and `std.task` async on it), so SysTick was set with raw addresses (`0xe000_e010`). The
   `Interrupts` struct has only `systick`: none of the chip's 52 interrupts can be handled, and
   there is no NVIC. No ADC, SPI, I2C, PWM, PIO, USB or flash driver. `Pin` has `output` and `uart`
   only: no input, no pull, no drive strength.
10. **`vun doc` does not document dependencies.** In the project it writes one namespace
    (`app`); `vun doc --library` writes 185 namespaces, none from `lib/boards`. The board's API
    (`rp2350.gpio.Pin.output`, `rp2350.rom.bootsel_hiding`) was found by reading source.
11. **Small diagnostics.** `int.does_not_fit` on `pin(40)` says "widen `Int(0..=29)` to hold the
    value", which is wrong advice for a type that mirrors the chip's 30 pins. The same error prints
    an absolute path in `vun build` and a relative one in `vun check app.vun`.
12. **Build output is untidy.** `.vun/build/` holds hash-named directories with `program.c`,
    `program.su` and `program.o`, and `app`, `app.ld`, `app.uf2` beside them; the useful files are
    the last three.

## What was good

- Typos get a `did you mean` that is right (`pinn` to `pin`, `bootsel_hidin` to `bootsel_hiding`).
- `vun check app.vun` works on a single file with the board as a dependency of the package.
- Builds are 0.1 to 0.2 s, and a release image for the program above is 644 bytes.
- The UF2 is accepted by the boot ROM, by the mounted drive and by `picotool load`.
- `.trap` and `.interrupts` as plain functions in `Config` made a custom trap and a SysTick handler
  a few lines each, and a `mut` static written from a handler and read from `main` worked.
- A fault and a bad ROM address are survivable now: the earlier report's silent hang is gone for
  faults.

## Left for the user to decide

1. Which first-run experience to build: a CYW43 LED, USB CDC, or only a documented board test.
2. Whether `rp2350.halt` should become "reboot to BOOTSEL", or a second trap is added.
3. A `vun flash` command (and `vun run` for firmware), or leaving flashing to `picotool` with a
   documented recipe.
4. A starter (`vun new --board rp2350`) and a corpus program.
