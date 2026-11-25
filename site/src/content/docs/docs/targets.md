---
title: "Targets"
description: "Choose what to build for, and see what each target needs."
---

A target says what a program is built for: the operating system or board, the architecture and the ABI, written as a path such as `linux.x86_64.gnu`. Each target has a backend that generates code, a linker, and a way to run what it makes.

## List them

```sh
vun target list
```

```ansi
[1m[32mtarget[0m                      [1m[32mbackend[0m  [1m[32mlinker[0m  [1m[32mruns[0m          [1m[32mstatus[0m
[1m[36mbare.thumbv7em.eabihf[0m       [1m[36mc[0m        [1m[36mgnu[0m     [2melsewhere[0m     [1m[32mready[0m
[1m[36mlinux.arm64.gnu[0m             [1m[36mnative[0m   [1m[36mgnu[0m     [1m[36mqemu-aarch64[0m  [1m[32mready[0m
[1m[36mlinux.riscv64.gnu[0m           [1m[36mc[0m        [1m[36mgnu[0m     [2melsewhere[0m     [1m[93mlacks [0m[2ma cross [0m[1m[36mcc[0m
[1m[36mlinux.x86_64.gnu[0m            [1m[36mnative[0m   [1m[36mgnu[0m     [1m[32mhere[0m          [1m[32mready[0m
[1m[36mmacos.arm64.darwin[0m          [1m[36mnative[0m   [1m[36mlld[0m     [2melsewhere[0m     [1m[32mready[0m
[1m[36mwasi.wasm32.p1[0m              [1m[36mwasm[0m     [1m[36mwasm[0m    [1m[36mwasmtime[0m      [1m[32mready[0m
[1m[36mwindows.x86.msvc[0m            [1m[36mc[0m        [1m[36mlld[0m     [2melsewhere[0m     [1m[32mready[0m
```

An excerpt: the list on your machine is longer and its `status` column is yours. `ready` means the toolchain for that target is installed here. `lacks <tool>` names what is missing, such as a cross compiler. `runs` says where the result runs: on `here`, under an emulator (`qemu-aarch64`, `wasmtime`), or `elsewhere`, on the device or machine it is for.

## Build for one

```sh
vun build --target wasi.wasm32.p1
vun run --target linux.arm64.gnu
```

`--target` (or `VUN_TARGET`) takes any path in the package's `targets`. Without it a build is for the machine it runs on.

## See what a build uses

```sh
vun target show main.vun
```

shows what a build for the target uses and where each part comes from.

## Backends and linkers

A target picks its backend and linker, and `--backend` and `--linker` override them for one build. `vun debug compare <file>` builds a program through every backend that generates code and reports whether they agree.

## Firmware

```sh
vun new blink --template rp2350
```

starts a project for a Raspberry Pi Pico 2. `vun flash` builds the programs a package's plan records and writes those it flashes to their devices.
