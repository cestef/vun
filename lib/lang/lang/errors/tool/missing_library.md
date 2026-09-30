# tool.missing_library

A command needs a native library that is not installed.

`vun` is one executable, and what it cannot hold without growing by tens of megabytes it loads
when a command first needs it, never at start (decision 131): libclang for `vun ffi bind`, libLLVM
for the LLVM backend. Every other command works without them.

## Example

```console
$ vun ffi bind sensor.h
error[tool.missing_library]: `vun ffi bind` needs libclang 9 or later, and none was found
```

## Fix

Install the library with the system's package manager (`clang-libs` on Fedora, `libclang-dev` on
Debian and Ubuntu, Xcode's command line tools on macOS), or point `LIBCLANG_PATH` at the directory
holding it.

```console
$ LIBCLANG_PATH=/usr/lib/llvm-18/lib vun ffi bind sensor.h
```
