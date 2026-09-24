# 08. Programs

Draft 1 had the build name a `program` declaration by string, and left open how a struct value
could generate a vector table or reach trap sites. Every system surveyed (research notes, section 4)
reduces entry to three pieces: a symbol the startup code calls, an entry symbol the linker starts
at, and a runtime written in the language. The magic, where there is any, is a name the compiler
knows (`main`, `#[panic_handler]`) or inspection of the root module (Zig's `@hasDecl(root, ...)`).
Overt keeps the three pieces and drops the magic.

## 1. Three rules

1. **Exports are the output.** A program is built from a root file. The compiler emits every
   `#[link.export]` declaration reachable from the root file's bindings, including those inside
   struct types the bindings evaluate to. Nothing else is special: no binding name, no signature
   and no build field names an entry point.
2. **The entry symbol is target data.** The `TargetSpec` names the linker's entry symbol (`_start`,
   `Reset`, `_initialize`, or none for a library). A missing or duplicated definition is a link
   error, which the compiler reports before linking, with the origin of each definition.
3. **Typed link symbols.** A declaration provided by the linker may be satisfied by an export in the
   same program. When both sides are Overt, their types must be identical, and the mismatch is a
   compile error. `lang.panic.trap`, allocator hooks and log sinks are all such symbols.

## 2. Runtimes are functions you call

A runtime is an ordinary compile-time function in an ordinary package. It takes your `main` (and
whatever else it needs) explicitly and returns a struct type holding the exports. You choose a
runtime by calling it; the binding's name is yours.

```ov
// hello/app.ov: a hosted program
start := std.os.start(main);

main := (sys: &mut std.os.System) -> std.os.Exit {
    mut out := sys.stdout();
    std.fmt.print(&mut out, "{} args\n", .{ sys.args().len() });
    .ok
};
```

```ov
// std/os/linux/start.ov: plain library code
start := ($main: (&mut System) -> Exit) -> type { struct {
    #[link.export("_start"), abi.naked]
    entry := () -> never {
        unsafe { lang.arch.asm("mov rdi, rsp; and rsp, -16; call {boot}", .{ .boot = _boot }) }
    };

    _boot := (sp: *usize) -> never {
        mut sys := unsafe { System.from_stack(sp) };
        exit(main(&mut sys))
    };

    #[link.export(lang.panic.trap_symbol)]
    trap := (t: &lang.panic.Trap) -> never { report(t); exit(.code(101)) };
}};
```

Linking against a C library instead is another runtime: `std.os.libc.start(main)` exports C `main`
and lets crt0 do the rest.

### Bare metal

```ov
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
- `#[link.section(".vectors"), link.export("__vectors")] vectors := ...`, a plain array of function
  pointers, with every unlisted interrupt filled by a default handler (no weak linker symbols);
- `#[link.export("Reset")] reset`, which copies `.data`, zeroes `.bss`, constructs `Peripherals`
  once through an `unsafe` constructor, and calls `main`;
- interrupt-ABI wrappers for each handler, and the `lang.panic.trap` export.

### WebAssembly, libraries, modules

```ov
start := wasi.p1.command(main);      // exports _start
start := wasi.cli.command(main);     // component model: exports wasi:cli/run#run
kmod  := linux.module(.{ .init = init, .exit = exit });   // init_module / cleanup_module + .modinfo
```

A library is a root file with `#[link.export]` declarations and no runtime.

## 3. Typed link symbols in detail

```ov
// lang/panic.ov
trap_symbol := "overt.trap";
#[link.symbol(trap_symbol)]
trap: (t: &Trap) -> never = _;
```

- Every trap site calls `lang.panic.trap`. The runtime's export satisfies it.
- A program with no provider fails to link with a diagnostic that names the missing symbol, its
  declared type, and the runtimes that would provide it.
- The same mechanism provides `std.log` sinks, `std.mem.alloc.global` for programs that opt into a
  global allocator, and test harness hooks. It is Rust's "externally implementable items" proposal,
  generalized and type-checked.
- Objects linked from outside Overt (a C runtime, a vendor library) are not type-checked; that is
  stated in the diagnostic when such a symbol is involved.

## 4. What the build does

The build names the root file, the target, and the output kind. It does not name declarations:

```ov
fw := b.program(.{ .root = "blinky/app.ov", .target = targets.thumbv7em.none_eabihf });
```

The root file is the only place where bindings are evaluated to discover exports; everywhere else
resolution stays lazy. This is one rule, stated here and in spec 13.
