# Vun

A systems programming language with nothing behind the scenes. Working name.

Status: design draft 2 and a compiler skeleton. Nothing compiles Vun yet.

```
docs/          specification, research notes, decision log
whitepaper/    the paper (Typst)
compiler/      the compiler, a Rust workspace
  base/        spans, origins, diagnostics, indices, queries, passes, tools, targets, tracing
  front/       grammar, syntax, HIR, sema
  mid/         MIR, MIR passes, reference interpreter
  low/         LIR, ABI, LIR passes
  back/        backend interface and backends (C, Cranelift, QBE, LLVM, process)
  link/        linker interface and adapters (wild, mold, lld, GNU ld, MSVC, process)
  render/      diagnostic renderers (short, annotate-snippets, ariadne, JSON, SARIF)
  tools/       driver, CLI, LSP, formatter, build, bindings, probe, harness, xtask
lib/           Vun sources: lang, std, targets, boards
corpus/        programs that must compile, and programs that must be rejected
design/        visual identity: palette, type, mascot, logo (see design/README.md)
```

```sh
cargo build                 # the toolchain
cargo nextest run           # tests
cargo run -p vun-cli      # the `vun` command
cargo gen                   # regenerate Rust from grammar, desugar, errors and lang, std
cargo xtask corpus          # corpus verdicts: pass, pending, fail
cargo xtask lines           # no source file past 300 lines
cargo xtask size            # the dist build of vun within its byte budget (decision 131)
cargo run -q -p vun-cli -- dump cst corpus/accept/integers/ring_buffer.vun
VUN_LOG=vun_sema=debug VUN_TIMINGS=1 cargo run -p vun-cli -- build
```

Start with [docs/spec/00-principles.md](docs/spec/00-principles.md).
