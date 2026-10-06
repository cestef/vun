# Open: backends, targets, runtimes

## wasm

- Inline assembly (`lang.arch.wasm32`, decisions 474, 501, 504 to 507) runs under wasmtime, with atomics
  and structured control (blocks giving a value, `br_table`). Calls name functions, and globals are
  the linker's (`stack_pointer`). SIMD is the proposal's vectors on the stack only (decision 507).
  Open: bound vectors (no Vun type is held as `v128`); `Check::Asm` does not apply
  to wasm blocks.
- The backend refuses naked functions and addresses past a function's start, and reports no frame
  sizes or debug information.
- A WASI stack overflow is the engine's raw memory fault unless built with `--checks stack` (473),
  which only the wasm backend and WASI's runtime provide.
- Where the system runs one thread (WASI), the first trap ends the run.

## Targets

- `macos.arm64` links through both backends but runs nowhere here; Mach-O arm64 thread-locals are
  checked by linking with `ld64.lld` only. No macOS backtraces (compact unwind, PAC).
- riscv64: `qemu-riscv64` exists, no sysroot; kernel runtime numbers are missing, and directories,
  seeking and environment.
- Windows runs through the C backend under Wine. The native backend needs a COFF code path and
  `Rules::WIN64` in `Rules::of`. A DLL is checked by linking, never by loading.
- stm32f4 past the blinky: serial, systick, clocks past the reset oscillator.
- Bare targets go through C; the native backend should be their default. ELF names `f1`/`d0`.
- ABI: legalize splits a soft `f64` by `Rules::fits`; `Rules` should be declared in `lang` and
  generated; no check that the convention's word matches `pointer_bits`.
- Backtraces: no call line per frame (native marks each call's origin; C cannot say where a call
  returns). WASI reports the site only.
- `vun debug disasm` names no data; ELF and PE readers are fuzzed for bounds, their agreement with
  llvm tools is checked by the driver's tests alone.
- Vectors: comparisons to `Mask(n)`, vector instructions in the backends, `std.simd.dispatch` over
  `#[target.require]`, the primitive `lang.simd.Scalable` (480 has `std.simd.Scalable` over
  `Spec.vector_bits` for now).
- Backend internals: regalloc loads spilled operands where the form takes memory; LIR has no
  undefined value (padding copied as zeros); fuzz target `lir.rs` is written, never run.
- Checks (268): a board and a WASI shadow, use after return; `fixed` kinds.

## rp2350 (`lib/boards/rp2350`)

Reports: `docs/reports/2026-10-04-pico2w*.md`. Built but never run on the board: the default trap,
NVIC and `Interrupts.external`, TIMER0, pin input, pulls and drive, `rp2350.flash.uf2`, the
template and README (decisions 431 to 440).

- The UART loopback byte arrives corrupt (`0xFC`, framing error); needs an adapter or probe. The
  pad's input was off until 442, so a receiver on GP1 is unrun.
- Missing: a CYW43 LED driver, ADC, SPI, I2C, PWM, PIO, a flash driver, the Hazard3 cores, `.dfu`
  and probe flash steps.

## Cranelift

- C records by value are lowered from `vun_abi::Passing` (decision 503). Still handed over: a record
  part of an odd size (3, 5, 6 or 7 bytes in a register), a struct argument not a multiple of 8
  bytes or aligned past 8, a signature AAPCS64 places differently from Cranelift's counting.
