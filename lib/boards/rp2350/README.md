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
-f -x`, which finds the board on the bus, then the drive the boot ROM shows once it is mounted.
BOOTSEL mode is entered by holding the button while plugging the board in.

## Seeing what a program did

A program that builds a `rp2350.usb.console.Console` shows up on the host as a serial port
(`/dev/ttyACM0`) and writes to it as any `std.fmt.Writer`; the `rp2350` template prints a line a
second. The USB controller is polled, so the console is served while the program waits through
`Console.delay`. The device's serial number is the chip's id, and it answers the vendor reset
request `picotool` sends, so `vun flash` replaces a running program without the button.

A program that trapped or faulted returns the board to BOOTSEL, passing what happened as the
reboot's two parameters (`rp2350.record`: kind, program counter, and the fault address, status or
trap site). Stock tools read it while the board waits:

```console
$ picotool info -a | grep "reboot param"
```

`rp2350.record.Words.decode` gives the `Stop` back. `.trap` of the program's configuration defaults
to `rp2350.fault.trap`; `rp2350.halt` stops the core for good, for a board a probe is attached to.
A program with no console can still say it ran by ending with
`rp2350.rom.bootsel_hiding(rp2350.rom.interface_mass_storage)`: the board comes back on the bus
without its drive.

## What is here

| Module | |
|--------|-|
| `rp2350` | `start(.{ .main, .trap, .interrupts })`, `Peripherals` |
| `rp2350.gpio` | pins as `output`, `input(pull)`, `uart`, and `drive` strength |
| `rp2350.time` | TIMER0: `now`, `delay`, and `after` for tasks (`std.task`) over alarm 0 |
| `rp2350.irq`, `rp2350.nvic` | the chip's 52 interrupts, enabled and cleared; handlers go in `Interrupts.external` |
| `rp2350.uart` | UART 0 at a rate on GP0 and GP1 |
| `rp2350.rom` | the boot ROM's functions: `reboot`, `bootsel`, `bootsel_hiding` |
| `rp2350.usb` | the USB device (`usb.device`) and the serial `usb.console` |
| `rp2350.record`, `rp2350.fault` | what a stop leaves behind, and how it is passed on |
| `rp2350.chip` | the chip's id |
| `rp2350.flash` | how `vun flash` reaches the board |
| `rp2350.layout` | the memory map and sections for `b.program` |

Not here: ADC, SPI, I2C, PWM, PIO, flash access from a program, the Hazard3 cores, and the
radio of the Pico 2 W. The console, the reset request, the stop record and `vun flash` ran on a Pico 2 W; the timer's
alarms, interrupts and input pins have been checked by building only.
