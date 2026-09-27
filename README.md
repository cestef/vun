# Vun

A systems programming language with nothing behind the scenes. Working name.

Vun is for software that runs under constraints: firmware, kernels, runtimes, servers. It is
built on seven concepts, each with one rule: bindings, compile-time values, range integers, place
states, scoped borrows, capabilities and suspension. Everything else is a library or a documented
rewrite into those.

```vun
start := std.os.start(main);

main := (sys: &mut std.os.System) -> std.os.Exit {
    mut out := sys.stdout();
    std.fmt.print(&mut out, "hello\n", .{});
    .ok
};

// Integers are ranges and arithmetic is exact: `a + b` is `Int(0..=8589934590)`,
// so this midpoint cannot overflow.
mid := (a: u32, b: u32) -> u32 { (a + b) / 2 };
```

A few things that follow from the rules:

- No overflow: arithmetic widens, and narrowing (`.to`, `.wrap`, `.sat`) is always written.
- No lifetime annotations: a returned borrow is tied to every argument it borrows.
- No destructors and no hidden calls: cleanup is `defer`, and a value that must be consumed is
  checked to be consumed on every path.
- No macro language: code generation is ordinary compile-time Vun, typed and traceable.
- No privileged code: primitives are declarations in the `lang` package, and `std` has no
  powers your code lacks. Even `main` is not special; the program exports `start`.

## Status

Experimental, and changing every day. Do not use it for anything that matters.

The compiler checks and runs real programs: parsing, types as values, compile-time evaluation,
place and borrow checks, and three backends (C, a native backend for x86-64 and arm64, and
WebAssembly). Programs run on Linux and under WASI. The standard library is small, bare-metal
targets are not there yet, and the language server does not exist.

## Try it

You need a recent Rust toolchain and a C compiler (used as the linker driver).

```sh
cargo run -q -p vun-cli -- run corpus/accept/programs/hello.vun
cargo run -q -p vun-cli -- check corpus/reject/integers/does_not_fit.vun
cargo run -q -p vun-cli -- --help
```

## Reading

- [The whitepaper](whitepaper/main.typ) (Typst: `typst compile whitepaper/main.typ`) is the best
  overview.
- [The specification](docs/spec/00-principles.md), one file per area, and the
  [decision log](docs/spec/decisions.md) with the reasoning behind each rule.
- [corpus/](corpus/) holds the programs the compiler must accept and those it must reject.

## Layout

```
docs/        specification, decision log, research notes
whitepaper/  the paper
compiler/    the compiler, a Rust workspace (base < front < mid < low < back, link < tools)
lib/         Vun sources: lang (primitives), std, targets
corpus/      test programs
design/      visual identity
```

Contributor commands and conventions are in [docs/HANDOFF.md](docs/HANDOFF.md).

## How this is made

The language design and its decisions are mine. Most of the code, the specification's prose and
the whitepaper were written by AI coding agents (Claude) working under my direction. Treat
everything here as a draft.
