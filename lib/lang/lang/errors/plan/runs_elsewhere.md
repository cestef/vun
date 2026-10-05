# plan.runs_elsewhere

`vun run` was asked to run a program built for a target this machine does not run.

A program for a device, such as a board's firmware, runs on the device, not here, and the
machine's tools file names no emulator for its target (`runner`, spec 12, section 6). The
program is built, and where its plan records a way to write it to the device, `vun flash` does so.

## Example

```console
$ vun run
error[plan.runs_elsewhere]: `.vun/build/app` is built for `bare.thumbv8m_main.eabihf`, which this machine does not run
  help: write it to the device with `vun flash`
```

## Fix

Run `vun flash` when the plan flashes the program (`b.flash`, spec 12, section 2), or name an
emulator for the target in the machine's tools file and run again.

```console
$ vun flash
    Flashing .vun/build/app.uf2
```
