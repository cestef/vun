# Overt

A systems programming language with nothing behind the scenes. Working name.

Status: design draft 2 and a compiler skeleton. Nothing compiles Overt yet.

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
lib/           Overt sources: lang, std, targets, boards
corpus/        programs that must compile, and programs that must be rejected
```

```sh
cargo build                 # the toolchain
cargo test                  # unit tests
cargo run -p overt-cli      # the `overt` command
cargo xtask generate        # regenerate Rust tables from grammar.ov and errors.ov
cargo xtask corpus          # corpus verdicts: pass, pending, fail
cargo xtask lines           # no source file past 300 lines
cargo run -q -p overt-cli -- dump cst corpus/accept/ring_buffer.ov
OVERT_LOG=overt_sema=debug OVERT_TIMINGS=1 cargo run -p overt-cli -- build
```

Start with [docs/spec/00-principles.md](docs/spec/00-principles.md).
