# rp2350

The Raspberry Pi RP2350 (Pico 2, Pico 2 W) on its Cortex-M33 cores: a runtime, the boot ROM, GPIO
and a timer, interrupts, a UART, and a way to write a program to the board.

## Start

```console
$ vun new blink --template rp2350
$ cd blink
$ vun build          # .vun/build/app, and app.uf2 beside it
$ vun flash          # with the board in BOOTSEL mode
```

`vun flash` gives the UF2 file to the board by the ways `rp2350.flash.uf2` lists: `picotool load
-x`, which finds the board on the bus, then the drive the boot ROM shows once it is mounted.
BOOTSEL mode is entered by holding the button while plugging the board in.

## Seeing what a program did

The board has no console. The on-board LED of a Pico 2 W is behind the radio chip, which has no
driver here, and the UART (`rp2350.uart`, GP0 and GP1) needs a serial adapter. Without one, a
program can say it ran by ending with `rp2350.rom.bootsel_hiding(rp2350.rom.interface_mass_storage)`:
the board comes back on the USB bus without its drive, which a program that never started does not
do (`rp2350.rom.bootsel()` shows the drive, as a board that was never flashed does).

A program that trapped or faulted also returns the board to BOOTSEL, having left what happened in
the watchdog's scratch registers: `rp2350.fault.take` reads it, as a `Stop` (a fault with its
address, status and instruction, or the site of a trap). `.trap` of the program's configuration
defaults to `rp2350.fault.trap`; `rp2350.halt` stops the core for good, for a board a probe is
attached to.

## What is here

| Module | |
|--------|-|
| `rp2350` | `start(.{ .main, .trap, .interrupts })`, `Peripherals` |
| `rp2350.gpio` | pins as `output`, `input(pull)`, `uart`, and `drive` strength |
| `rp2350.time` | TIMER0: `now`, `delay`, and `after` for tasks (`std.task`) over alarm 0 |
| `rp2350.irq`, `rp2350.nvic` | the chip's 52 interrupts, enabled and cleared; handlers go in `Interrupts.external` |
| `rp2350.uart` | UART 0 at a rate on GP0 and GP1 |
| `rp2350.rom` | the boot ROM's functions: `reboot`, `bootsel`, `bootsel_hiding` |
| `rp2350.fault` | what a stop leaves behind |
| `rp2350.flash` | how `vun flash` reaches the board |
| `rp2350.layout` | the memory map and sections for `b.program` |

Not here: ADC, SPI, I2C, PWM, PIO, USB, flash access from a program, the Hazard3 cores, and the
radio of the Pico 2 W. Everything past bring-up (the timer, interrupts, input pins) has been
checked by building only; the board itself ran the trap, fault, SysTick and UART-less programs of
`docs/reports/2026-10-04-pico2w-new-user.md`.
