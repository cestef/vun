# cli.unavailable

A command the toolchain declares is run from a program built without the part that runs it.

The commands of `vun` are declared once, in `vun-driver`; a part the driver cannot depend on,
such as the language server (`vun-lsp`, spec 13, section 11), is installed by the `vun` binary.
A program embedding the driver without installing it declares the command but cannot run it.

## Example

```console
$ my-embedded-driver util lsp
error[cli.unavailable]: this program runs without `util lsp`
```

## Fix

Run the command from `vun`, or install the part in the program as `vun` does:

```console
$ vun util lsp
```
