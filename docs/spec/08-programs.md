# 08. Programs

Draft 1 had the build name a `program` declaration by string, and left open how a struct value
could generate a vector table or reach trap sites. Every system surveyed (research notes, section 4)
reduces entry to three pieces: a symbol the startup code calls, an entry symbol the linker starts
at, and a runtime written in the language. The magic, where there is any, is a name the compiler
knows (`main`, `#[panic_handler]`) or inspection of the root module (Zig's `@hasDecl(root, ...)`).
Vun keeps the three pieces and drops the magic.

## 1. Three rules

1. **Exports are the output.** A program is built from a root file. The compiler emits every
   `#[link.export]` declaration reachable from the root file's bindings, including those inside
   struct types the bindings evaluate to. Nothing else is special: no binding name, no signature
   and no build field names an entry point.
2. **The entry symbol is target data.** The `TargetSpec` names the linker's entry symbol (`_start`,
   `Reset`, `_initialize`, or none for a library). A missing or duplicated definition is a link
   error, which the compiler reports before linking, with the origin of each definition.
3. **Typed link symbols.** A declaration provided by the linker may be satisfied by an export in the
   same program. When both sides are Vun, their types must be identical, and the mismatch is a
   compile error. `lang.panic.trap`, allocator hooks and log sinks are all such symbols.

## 2. Runtimes are functions you call

A runtime is an ordinary compile-time function in an ordinary package. It takes your `main` (and
whatever else it needs) explicitly and returns a struct type holding the exports. You choose a
runtime by calling it; the binding's name is yours.

```vun
// hello/app.vun: a hosted program
start := std.os.start(main);

main := (sys: &mut std.os.System) -> std.os.Exit {
    mut out := sys.stdout();
    std.fmt.print(&mut out, "{} args\n", sys.args().len());
    .ok
};
```

```vun
// std/os/linux/kernel.vun: plain library code, abridged
start := ($main: (&mut System) -> Exit) -> type { struct {
    #[link.export("_start"), abi.naked]
    entry := () -> never {
        unsafe {
            lang.arch.asm({
                ..lang.arch.x86_64.names;
                mov(rdi, rsp);
                \and(rsp, -16);
                call(_boot);
            });
            lang.hint.unreachable()
        }
    };

    _boot := (sp: *usize) -> never {
        mut sys := unsafe { System.from_stack(sp) };
        exit(main(&mut sys))
    };

    #[link.export(lang.panic.symbols.trap)]
    trap := (t: &lang.panic.Trap) -> never { std.trace.trapped(t, trap); exit(.code(101)) };
}};
```

That runtime, `std.os.linux.kernel.start`, calls the kernel itself: every request is a system call
through a block (`syscall`, `svc 0`), the shadow of the checks of unsafe code is mapped with `mmap`,
and it exports the four functions of C's `string.h` a freestanding C compiler may call (`memcpy`,
`memmove`, `memset`, `memcmp`), written in assembly so that no C compiler reads them back as calls
of themselves. On 32-bit Arm it also exports the EABI's hooks for a division by zero
(`__aeabi_idiv0`, `__aeabi_ldiv0`), which the C compiler's division functions call and a C
library's defaults answer with `raise`; Vun checks its divisors, so reaching one is a trap. The
program is static: linked with `-static -nostdlib`, naming only the C compiler's helper library
(`libgcc`) after its objects (`lang.target.CCompiler.libraries`), with no C library, start files
or unwinder, which a program without unwinding never calls. Linking against a C library instead is another runtime:
`std.os.libc.start(main)` exports C `main` and lets crt0 do the rest. What each C library lays out its own way (the function giving
`errno`, the signal set, the values of `pthread_sigmask`) is a row per system,
`std.os.libc.System`, which each system's module declares (`std.os.linux.c`); the runtime takes
the row whose `os` is `lang.target.current.os`.

**Choosing a runtime.** `std.os.start` is not a runtime of its own: it is the `start` of the
runtime of the target being built for. Each system's namespace declares its row,
`std.os.Runtime { os, abi, start }` (`std.os.linux.runtime` is the kernel runtime for
`linux`/`kernel`, `std.os.linux.gnu` the C library's for `linux`/`gnu`, `std.os.wasi.runtime` WASI
preview 1's for `wasi`/`p1`, `std.os.windows.runtime` the one calling `kernel32` for
`windows`/`msvc`), and `std.os.runtime` takes
the row matching `lang.target.current`'s system and ABI; a target none matches fails when
compiling. So no program, and nothing in `std` but a runtime, is tied to a C library: `std.os`
reaches the system through typed link symbols each runtime exports (`std.os.symbols.write`, beside
`lang.panic.symbols.trap` and `std.sync.symbols.enter`).

**Windows.** A program for `windows.x86_64.msvc` is a PE file linked by `lld-link` against the
import libraries in the target's sysroot (`kernel32.lib`, which the machine's tools file names:
the Windows SDK's, xwin's or mingw-w64's) and calls `kernel32` itself, with no C runtime, through
the typed imports of `std.os.windows.kernel32`. It runs one thread (`lang.target.Spec.threads` is
`one`, `spawn` is `unsupported`). A file number is a handle, which a 64-bit system uses the low 32
bits of, and 0, 1 and 2 are the standard streams, no handle being one of them (handles are
multiples of 4); paths are UTF-8 in the program and UTF-16 to the system (`std.text.utf16`); the
command line is one UTF-16 string that the program splits as the C runtime does
(`std.os.windows.command_line`) into every argument the longest command line the system gives
(32767 units) can hold, a surrogate without its partner being U+FFFD as invalid UTF-8 is, and the
real clock set before 1970 reads as the epoch; a console is set to UTF-8 at the start. The runtime provides the
functions a freestanding C compiler calls (`std.os.memory`, and `__chkstk`, which probes the
pages of a large frame in order, `std.os.windows.stack`).

`windows.x86.msvc` is the same for 32-bit x86: `kernel32`'s functions are `stdcall` (the system's
convention there, which x86-64 takes for its one, `lang.target.Spec.accepts`, so
`#[abi.stdcall]` is written once for both), a handle is the whole 32 bits, and what the C
compiler calls for (the division of 64-bit integers, `_chkstk`, the copies and fills) is
`ntdll`'s, which the target links after `kernel32`, no assembler for 32-bit x86 being written
yet. A function of a library or a program names its convention as C does (`#[abi.fastcall]`,
`#[abi.thiscall]`), and a function type carries it, so that `lang.mem.function_at` makes a
callable function of an address of the code of a C++ method.

**Clocks.** `std.time` holds spans of time (`Duration`, to the nanosecond) and readings of a clock
(`Instant`, nanoseconds since the clock's start); `Clock` is `monotonic` (never goes back, from an
instant the system chose) or `real` (the time of day since 1970 UTC). `System.now(clock)` reads
one and `System.pause(span)` suspends the thread for at least `span`, through the link symbols
`std.os.symbols.{now, pause}`: the kernel runtime calls `clock_gettime` and `clock_nanosleep`
itself, the C library's runtime its functions of the same names (`nanosleep` to pause), each
system's row giving its clocks' numbers (`std.os.Clocks`: 1 and 0 on Linux, 6 and 0 on macOS),
and WASI's `clock_time_get` and `poll_oneoff` on a clock. A pause a signal interrupts goes on with
what is left.

**Threads.** `System.spawn(&mut f)` starts a thread running the closure `f` (any
`lang.ops.Call((), R)`) and gives a `std.os.thread.Thread(R, F)`: linear, and tied to the borrow
of `f`, so the closure and what it borrows stay, and are not reached otherwise, until `join`,
which consumes the thread and gives what `f` returned, or `.err(trap)` when a trap ended the
thread (spec 07, section 3); a thread never joined is
`place.linear_leak`, a closure reached while its thread runs `borrow.conflict`. Values safe to
share are safe across threads (spec 07, section 2), so threads borrow what the one starting them
goes on reading. A runtime starts a thread through `std.os.symbols.{spawn, join}` with a
function of `std.os.thread` and a copy of the thread's context it keeps until the join copies it
back: the kernel runtime maps each thread a stack of 8 MiB, its lowest 64 KiB left inaccessible,
and calls `clone` sharing memory, files and signal handlers, the new thread jumping to a function
that runs it and ends it with `exit`; joining waits on the thread's id, which the kernel clears
when it ends (`CLONE_CHILD_CLEARTID`, then `futex`), and unmaps the stack. The C library's runtime
calls `pthread_create` and `pthread_join`; WASI's, whose module runs one thread, reports
`Error.unsupported`. Each thread has its own storage of the bindings a type marks
`#[thread.local]` (spec 07, section 2): the C library lays it out; the kernel runtime finds the
program's TLS segment through the auxiliary vector (`AT_PHDR`), lays each thread's block out as
a table per architecture says (x86-64's below the thread pointer, whose first word points to
itself; arm64's and Arm's above a header of 16 or 8 bytes), sets the main thread's pointer when
it starts (`arch_prctl`, `tpidr_el0`, `set_tls`), and each thread it starts sets its own as the
first thing it does, the block placed in the thread's mapping.

**Libraries.** `System.load(path)` loads a shared library as the program runs and gives a
`std.os.library.Library`, which names the file or the library the system finds by that name.
`unsafe library.function(F, name)` is what it exports as `name`, as a function of the type `F`,
`#[abi.c]` for C's: made of its address (`lang.mem.function_at`), so its code must be written to
`F`, and it must not be called after `close`, which unloads the library. A runtime reaches the
loader through `std.os.library.symbols.{load, symbol, unload}`: the C library's calls `dlopen`
(`RTLD_NOW` and `RTLD_LOCAL`, as `std.os.libc.System.load` numbers them), `dlsym` and `dlclose`, and
the Windows runtime `LoadLibraryW`, `GetProcAddress` and `FreeLibrary`; the kernel runtime and
WASI's, which load nothing, report `Error.unsupported`. A library the loader does not find or
cannot load, or a name it does not hold, is `Error.unresolved`.

**Heap and environment.** `std.os.heap.Heap` is an `Allocator` over the heap a runtime exports
(`std.os.heap.symbols.{take, give}`, by size and alignment): the C library's runtime calls `malloc`
and `free` (an alignment past what `malloc` keeps is `.exhausted`), and the kernel, WASI and
Windows runtimes, which have none yet, refuse every allocation with `.exhausted`. `System.env(name)`
is the value of an environment variable, none when it is not set, through
`std.os.env.symbols.get`: `getenv` for the C library's runtime, nothing set for the others.

**Files and errors.** `System.open(path, mode)` gives a `std.os.file.File` (`Mode`: `read`,
`write` creating or emptying, `append` creating, `update`), whose `stream()` is read
(`Stream.read`, 0 at the end) and written as any `Stream`, and `close` ends it; `System.stdin()`
is the stream 0. They reach the system through `std.os.symbols.{open, read, close}`: the kernel
runtime calls `openat` from the working directory, `read` and `close`; the C library's runtime
its functions (C's `open` is variadic, its mode passed always as its pack, spec 07, section 5);
WASI opens from the preopened directory a path is inside (`.` for a
relative one), `.denied` when the host gave none. A path is passed with a zero after it
(`std.os.file.Path`, up to `PATH_MAX`), every file closed in a program the process runs next, a
created one read and written by its owner. A failure is a `std.os.Error`: a reason `std` names
(`missing`, `denied`, `exists`, `directory`, `not_directory`, `path`, `interrupted`, `closed`) or
the system's own number (`number(n)`), each system mapping its numbers (`std.os.posix_error` for
Linux and macOS, which number a path too long apart; `std.os.wasi.error`).

**Linux targets.** `targets.linux.<arch>.kernel` (`linux.x86_64.kernel`, `linux.arm64.kernel`)
builds static programs on the kernel runtime, with ABI `kernel`; `targets.linux.<arch>.gnu`
builds programs on the GNU C library, with ABI `gnu`, which a program calling C picks. arm64's static target compiles C without
out-of-line atomics, which ask the C library what the CPU has.

**Exit statuses.** How a program ends is `std.os.Exit`: `ok`, or `code(s)` with `s` a
`std.os.Status`, `Int(1..=125)`. The range is what every system `std` runs on passes on
unchanged: a shell reads 126 and 127 as a program that could not run and 128 plus `n` as one
killed by signal `n`, and WASI's `proc_exit` refuses 126 and above. So `.code(200)` does not type
check, the same as any value outside its integer's range (spec 04), and no runtime checks a
status: each passes `Exit.status()`, 0 for `ok`, to its system as it is, and `Exit.of(n)` is its
inverse for a computed `n: Int(0..=125)`. A trap ends with `std.os.trapped`, a `Status` (101). A system with statuses of its own (a Windows `NTSTATUS`) is
reached through that system's namespace, not through `Exit`.

### Bare metal

```vun
start := stm32f4.start(.{
    .main = main,
    .trap = stm32f4.halt,
    .interrupts = .{ .systick = on_systick, .tim2 = stm32f4.time.on_tim2 },
});

main := (p: stm32f4.Peripherals) -> never {
    mut led := p.gpioa.pin(5).output();
    mut timer := stm32f4.time.Timer.init(p.tim2);
    mut a := blink(&mut led, &timer);     // frames built in place in main's stack frame
    std.task.run(.{ &mut a })             // -> never, so the frames live forever
};
```

`stm32f4.start` builds, at compile time:
- `#[link.section(".vectors"), link.export("__vectors")] vectors := ...`, a plain `#[layout.c]`
  struct: the stack pointer the core starts with, pointing to `__ram_end` (a link symbol's
  address is a constant, whether pointed to or borrowed), then a handler per vector, every
  vector not given run by a default handler (no weak linker symbols);
- `#[link.export("Reset")] reset`, which copies `.data` from where it loads and zeroes `.bss`, word
  by word from the symbols the linker defines at their bounds, constructs `Peripherals` once
  through an `unsafe` constructor, and calls `main`;
- interrupt-ABI wrappers for each handler, and the `lang.panic.trap` export.

An exported value that is not a function, as the vector table, is data of the program: the
functions and link symbols it holds are reached from it, and it is placed in its link section.
One whose value has none at compile time is an error (`comptime.export`), reported at it and,
when evaluation stopped in a function it calls, where it stopped.
Where it lies comes from the build: `stm32f4.layout` gives the chip's `memory` and the `sections`
placed in it, which the program's build passes (spec 12, section 2); the linker defines
`__<section>_start`, `__<section>_end` and, for a section loaded from another region,
`__<section>_load`, and `__<region>_end` for each region. `stm32f4.semihost.exit` ends a program
under a debugger or an emulator with a status (Arm's semihosting): the driver's firmware test runs on qemu's
STM32F405 board.

`Peripherals` holds a token per peripheral the board gives, made only by its `unsafe` claim:
`gpioa` to `gpioc` (`stm32f4.gpio.Port`), whose `pin(n).output()` turns the port's clock on and
gives an `Output` to `toggle`, and `tim2` (`stm32f4.time.Tim2`), which `Timer.init` takes, counting
milliseconds. `Timer.after(d)` starts the counter for one update and waits, through
`std.task.Event.wait`, on the event `stm32f4.time.on_tim2`, the handler a program names for the
interrupt, fires. Registers are `std.hw.Reg` fields of a `#[layout.c]` struct borrowed at the
device's address (`std.hw.at`). The firmware test runs the program above, stopping after three
waits.

### WebAssembly, libraries, modules

```vun
start := wasi.p1.command(main);      // exports _start
start := wasi.cli.command(main);     // component model: exports wasi:cli/run#run
kmod  := linux.module(.{ .init = init, .exit = exit });   // init_module / cleanup_module + .modinfo
```

A library is a root file with `#[link.export]` declarations and no runtime: built as
`std.build.Output.shared` or `.static` (a plan's `Options.output`, or `vun build --library
shared`), it has no entry symbol, and its file is named as its object format names libraries
(`libcalc.so` and `libcalc.a` on ELF, `calc.dll` and `calc.lib` on PE, `calc.wasm` on
WebAssembly). A shared library binds to its own definitions: its exports are not replaced by
another module's, and a PE file names each of them in its export table. A library holds no
runtime, so where the code its compiler makes assumes what a C runtime defines, the library holds
what provides it: on Windows `services := std.os.windows.services`, the services of the runtime
beside its start (the trap, the operations of `std.os`, `memcpy`, `__chkstk`, `_fltused`), which
a DLL has to define itself because nothing loads a C runtime for it.

## 3. Typed link symbols in detail

```vun
// lang/panic.vun
symbols := struct {
    trap := "vun_trap";
};
#[link.symbol(symbols.trap)]
trap: (t: &Trap) -> never = _;
```

- A namespace's link symbol names are the members of its `symbols` struct, beside the
  declarations naming them (`lang.panic.symbols.trap`, `std.sync.symbols.enter`); a runtime
  exports under the same names.

- Every trap site calls `lang.panic.trap`. The runtime's export satisfies it; a hosted runtime's
  writes the trap's backtrace to the standard error first (`std.trace`, spec 07, section 3),
  reading the table of the program's code, whose link symbol it declares
  (`std.trace.table`) and the compiler defines.
- A program with no provider fails to link with a diagnostic that names the missing symbol, its
  declared type, and the runtimes that would provide it.
- The same mechanism provides `std.log` sinks, `std.mem.alloc.global` for programs that opt into a
  global allocator, and test harness hooks. It is Rust's "externally implementable items" proposal,
  generalized and type-checked.
- Objects linked from outside Vun (a C runtime, a vendor library) are not type-checked; that is
  stated in the diagnostic when such a symbol is involved.

## 4. What the build does

The build names the root file, the target, and the output kind. It does not name declarations:

```vun
fw := b.program(.{ .root = "blinky/app.vun", .target = targets.bare.thumbv7em.eabihf });
```

The root file is the only place where bindings are evaluated to discover exports; everywhere else
resolution stays lazy. This is one rule, stated here and in spec 13.
