# 14. Roadmap

| Phase | Deliverable | Exit criterion |
|-------|-------------|----------------|
| 0 | This spec; the grammar file; a corpus of about 40 programs (embedded, hosted, async, FFI, codegen) and 40 must-fail programs, written before the compiler; the whitepaper draft | every corpus program is explained by the concept list and appendix B alone |
| 1 | `vun-syntax` (from `grammar.vun`), `hir`, `sema`, `interp`, query engine with persistence, origins, `vun-diag` with the terminal renderer; `lang` and a minimal `std` | the corpus type-checks; generics, interfaces, reflection, L0 to L3 metaprogramming run; lines per second tracked in CI |
| 2 | MIR checks: place states, kinds, `defer`, borrows, scoping; coroutine lowering; must-fail corpus | every must-fail file is rejected with exactly its expected diagnostics; generators and async run in the interpreter |
| 3 | LIR, C backend, `Linker` with lld and GNU ld, QEMU runner, the test runner in all modes | blinky on a real board; differential tests against the interpreter pass |
| 4 | Cranelift and QBE backends, wild and mold, hosted `std.os`, benchmarks and fuzzing | the compiler's test suite passes natively on three backends |
| 5 | board runtimes, `std.task` executors, stack and `panic.never` analyses, deferred logging, `vun util ffi bind`, probe-rs integration | measured stack high-water marks within computed bounds; Vun inside an ESP-IDF project |
| 6 | `fmt`, LSP, `fix`, lints API, package tooling, `doc`, LLVM backend | usable daily; code size within 10% of C on a reference firmware |
