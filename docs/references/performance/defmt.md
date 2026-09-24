![](assets/knurling_logo_light_text.svg)

# [`defmt`](#defmt)

> `defmt` (“de format”, short for “deferred formatting”) is a highly efficient logging framework that targets resource-constrained devices, like microcontrollers.

## [Features](#features)

- `println!`-like formatting
- Multiple logging levels: `error`, `info`, `warn`, `debug`, `trace`
- Compile-time `RUST_LOG`-like filtering of logs: include/omit logging levels with module-level granularity
- Timestamped logs

## [Current limitations](#current-limitations)

- Output object format must be ELF
- Custom linking (linker script) is required
- Single, global logger instance (but using multiple channels is possible)

## [Intended use](#intended-use)

In its current iteration `defmt` mainly targets tiny embedded devices that have no means to display information to the developer, e.g. a screen. In this scenario logs need to be transferred to a second machine, usually a PC/laptop, before they can be displayed to the developer/end-user.

`defmt` operating principles, however, are applicable to beefier machines and could be used to improve the logging performance of x86 web server applications and the like.

## [Operating principle](#operating-principle)

`defmt` achieves high performance using deferred formatting and string compression.

Deferred formatting means that formatting is not done on the machine that’s logging data but on a second machine. That is, instead of formatting `255u8` into `"255"` and sending the string, the single-byte binary data is sent to a second machine, the *host*, and the formatting happens there.

`defmt`’s string compression consists of building a table of string literals, like `"Hello, world"` or `"The answer is {:?}"`, at compile time. At runtime the logging machine sends *indices* instead of complete strings.

## [Components](#components)

Fundamentally, `defmt` is comprised of multiple packages - some run on your microcontroller 📟, some run on your host machine 🖥️, and some are macros that generate code (for either of those scenarios) 🚧.

### [](#defmt-)[`defmt`](https://crates.io/crates/defmt) 📟

The `defmt` crate runs on your microcontroller or other target device. It exports macros, like `info!` that libraries can use to emit logging messages, and the `#[no_mangle]` infrastructure those macros use to send log messages to the registered logging transport.

The `defmt` crate requires a *transport* to be registered within your firmware. Example transports include `defmt-rtt` and `defmt-itm`. The transport is handed a *log frame* every time a line of code like `defmt::info!("...")` is executed. That *log frame* describes which interned format string to use, and what arguments to print with it.

### [](#defmt-rtt-)[`defmt-rtt`](https://crates.io/crates/defmt-rtt) 📟

This library is a *logging transport* for `defmt` that sends data over SEGGER’s RTT transport protocol.

This is a good choice when using `probe-rs` because support is built-in to that runner.

### [](#defmt-itm-)[`defmt-itm`](https://crates.io/crates/defmt-itm) 📟

This library is a *logging transport* for defmt that sends data over Arm’s Instruction Trace Macrocell.

This might be a good choice if you are using a commercial debugger with ITM support but not RTT support.

### [](#defmt-semihosting-)[`defmt-semihosting`](https://crates.io/crates/defmt-semihosting) 📟

This library is a *logging transport* for defmt that sends data over *semihosting* calls (i.e. breakpoints that wake up your debugger).

You should only use this when running firmware inside QEMU, because otherwise it’s very slow.

### [](#defmt-test-)[`defmt-test`](https://crates.io/crates/defmt-test) 📟

This library is for running unit tests with our deprecated runner `probe-run`. You might want to look at [`embedded-test`](https://crates.io/crates/embedded-test), which integrates a bit better with `probe-rs`.

### [](#panic-probe-)[`panic-probe`](https://crates.io/crates/panic-probe) 📟

This library can be added to an Embedded Rust application to provide an implementation of `#[panic_handler]`. It can optionally route the `core::panic::PanicInfo` structure over RTT (using the `rtt_target` crate) or over defmt (using the `defmt` crate).

### [](#defmt-decoder-)[`defmt-decoder`](https://crates.io/crates/defmt-decoder) 🖥️

The `defmt-decoder` library turns *log frames* into human-readable Unicode text. The popular `probe-rs` runner uses this library to decode `defmt` log frames emitted by your firmware.

### [](#defmt-parser-)[`defmt-parser`](https://crates.io/crates/defmt-parser) 🖥️

This library turns defmt log frames into Rust structures. You probably want to use `defmt-decoder` instead, which actually decodes the log frames instead of just parsing them.

### [](#defmt-print-)[`defmt-print`](https://crates.io/crates/defmt-print) 🖥️

The `defmt-print` CLI program uses `defmt-decoder` to turn `defmt` log frames into human-readable Unicode text. You can use this if your log frames aren’t coming via `probe-rs` but instead come in through some other mechanism (e.g. a network connection).

### [](#defmt-macros-)[`defmt-macros`](https:///crates.io/crates/defmt-macros) 🚧

This crate contains the procedural macros used and/or exported by the `defmt` crate. It is an internal implementation detail and should be not used standalone.

## [Support](#support)

`defmt` is part of the [Knurling](https://knurling.ferrous-systems.com/) project, [Ferrous Systems](https://ferrous-systems.com/)’ effort at improving tooling used to develop for embedded systems.

If you think that our work is useful, consider sponsoring it via [GitHub Sponsors](https://github.com/sponsors/knurling-rs).

[![@knurling-rs](https://avatars.githubusercontent.com/u/23323981?s=96&v=4)](/knurling-rs)

Sponsor Knurling on GitHub Sponsors

We build modern tooling, libraries, and learning material for embedded systems. We make it easy to get a grip on bare metal!

[ ![](data:image/svg+xml;base64,PHN2ZyBhcmlhLWhpZGRlbj0idHJ1ZSIgZGF0YS1jb21wb25lbnQ9Ik9jdGljb24iIGhlaWdodD0iMTYiIHZpZXdib3g9IjAgMCAxNiAxNiIgdmVyc2lvbj0iMS4xIiB3aWR0aD0iMTYiIGRhdGEtdmlldy1jb21wb25lbnQ9InRydWUiIGNsYXNzPSJvY3RpY29uIG9jdGljb24taGVhcnQgY29sb3ItZmctc3BvbnNvcnMiPgogICAgPHBhdGggZD0ibTggMTQuMjUuMzQ1LjY2NmEuNzUuNzUgMCAwIDEtLjY5IDBsLS4wMDgtLjAwNC0uMDE4LS4wMWE3LjE1MiA3LjE1MiAwIDAgMS0uMzEtLjE3IDIyLjA1NSAyMi4wNTUgMCAwIDEtMy40MzQtMi40MTRDMi4wNDUgMTAuNzMxIDAgOC4zNSAwIDUuNSAwIDIuODM2IDIuMDg2IDEgNC4yNSAxIDUuNzk3IDEgNy4xNTMgMS44MDIgOCAzLjAyIDguODQ3IDEuODAyIDEwLjIwMyAxIDExLjc1IDEgMTMuOTE0IDEgMTYgMi44MzYgMTYgNS41YzAgMi44NS0yLjA0NSA1LjIzMS0zLjg4NSA2LjgxOGEyMi4wNjYgMjIuMDY2IDAgMCAxLTMuNzQ0IDIuNTg0bC0uMDE4LjAxLS4wMDYuMDAzaC0uMDAyWk00LjI1IDIuNWMtMS4zMzYgMC0yLjc1IDEuMTY0LTIuNzUgMyAwIDIuMTUgMS41OCA0LjE0NCAzLjM2NSA1LjY4MkEyMC41OCAyMC41OCAwIDAgMCA4IDEzLjM5M2EyMC41OCAyMC41OCAwIDAgMCAzLjEzNS0yLjIxMUMxMi45MiA5LjY0NCAxNC41IDcuNjUgMTQuNSA1LjVjMC0xLjgzNi0xLjQxNC0zLTIuNzUtMy0xLjM3MyAwLTIuNjA5Ljk4Ni0zLjAyOSAyLjQ1NmEuNzQ5Ljc0OSAwIDAgMS0xLjQ0MiAwQzYuODU5IDMuNDg2IDUuNjIzIDIuNSA0LjI1IDIuNVoiIC8+Cjwvc3ZnPg==) Sponsor ](/sponsors/knurling-rs?o=esc)

` commit dfe53ad on 2026-09-22 14:37:33 `
