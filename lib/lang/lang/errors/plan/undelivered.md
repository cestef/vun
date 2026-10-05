# plan.undelivered

A file made for a device could not be given to it.

A device taking a file of blocks (`std.build.Flash.uf2`) is reached through each of the ways this
toolchain knows, in turn: `picotool`, when it is installed, and the drive the device shows when
it is in its boot loader mode, when it is mounted. This is reported when none reached a device;
each way says why it did not.

## Example

```console
$ vun flash
error[plan.undelivered]: cannot write `.vun/build/app.uf2` to a device
  note: `picotool` is not on the search path
  note: no mounted drive holds `INFO_UF2.TXT`
  help: put the device in its boot loader mode and run `vun flash` again
```

## Fix

Put the device in its boot loader mode (a Raspberry Pi Pico: hold BOOTSEL while plugging it in),
then install `picotool`, or mount the drive the device shows.

```console
$ vun flash
    Flashing .vun/build/app.uf2
```
