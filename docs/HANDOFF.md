# Handoff

State of Vun as of 2026-09-30, for the next agent: what is open and what comes next. Nothing here
is durable; conventions and how-tos live in [`dev/`](dev/workflow.md), rules in
[`spec/`](spec/00-principles.md), history in `jj log` and [`decisions.md`](spec/decisions.md).
Read `CLAUDE.md` and [`dev/workflow.md`](dev/workflow.md) first.

## State

Everything is committed in the main chain; no `jj` workspace holds work. 671 tests pass; clippy,
`lines` and `layers` are clean; the corpus has 309 passing programs and 3 planned, and LIR
lowers every function the corpus reaches.

Built: the front end (a parser reading the productions of `nodes.vun`), packages and the store,
the persisted parallel query engine, sema, MIR and its checks, LIR with legalization, the C and
wasm backends, the native backend for x86-64, arm64 and Thumb-2, ELF and Mach-O objects, the
`gnu`, `mold` and `wild` linkers, the toolchain file, static Linux runtimes calling the kernel
(threads, storage per thread, files, clocks), backtraces, lints (`lang.lint.Lint`,
`std.lints.naming.case`), inline assembly as calls of `lang.arch` declarations, editor packages
generated from the grammar (tree-sitter, Helix, Neovim, Zed, VS Code) and the language server.
Programs run through either backend on linux.x86_64, linux.arm64, linux.arm (through `qemu`) and
wasi.wasm32.

Last landed: the performance pass (348). Criterion benchmarks (`cargo bench -p
vun-syntax|vun-driver`, baselines `before` and `pinned` saved on `am3.wg`), `--timeline <file>`
for Perfetto, longest runs and waits in `--timings`, `cargo xtask remote --reserve` for stable
measurements (`dev/commands.md`, "Measuring speed"). On `am3.wg`: cold check 0.41 s to 0.16 s
for `tour`, check from saved memos −20%, a build after an edit instantiating library generics
149 ms to 66 ms, parsing 7 times faster. A shared memo file is now written once per toolchain,
target and checks, by the first session finding none. Before that (344 to 347): a
`lang.diag.error` reached at run time refuses the program at its call, impls checked where their
witness is made, spread members of type bodies, editor queries last-wins.

Brand kits (342, [`dev/assets.md`](dev/assets.md)) open points: removing outputs of variants no
longer declared, as `cargo gen` does; code in the tall and square clips is small at 80 columns;
the OBS overlays are not yet tried in OBS itself.

## Decisions waiting for the owner

Proposed, not yet accepted: 249 to 251, 253 to 257, 259, 260, 262, 265, 266, 268, 269, 271, 273
(integers in patterns), 274 (`$` forms that cannot run where written), 275 (`$if` branch scopes),
276 (C naked functions), 277 (ELF32), 278 (`layout.offset`), 280 (32-bit AAPCS in `vun-abi`), 281
(clocks), 283 (query soundness), 284 (files, typed errors, builds checking library bodies), 316
(JSON derives), 317 (`vun-uri`, typed manifest URLs), 318 (typed LSP, its new features), 319
(productions), 320 (`vun-data`), 321 (editor grammars and packages), 322 (declaration origins
counted by name), 323 (the grammar read by type into one model, `editors.vun`, operand contexts,
the parser reading the tables), 324 (language server features), 325 (DWARF subprograms), 327
(exported data), 328 (a constant shifted by a run-time amount has no width), 329 (board
peripherals, exported aliases, `libgcc` on bare targets), 330 (macOS linking from target data,
`sysroot`, `os_version`), 340 (the parser over the productions, and the grammar's edge cases
gone), 341 (the checks' shadow reserved span by span), 344 (`lang.diag.error` at run time), 346
(spread members of type bodies), 347 (editor queries last-wins, types captured, server fixes). 279 (static Linux targets), 282 (`&mut T` where `&T` is), 326
(section placements) and 345 (impls checked where their witness is made) were the owner's choice.

## Known gaps in landed work

- A `$`-generic C import (a pack inferred per call) is typed nowhere yet: such calls stay
  silently untyped (decisions 285, 286).
- ABI: legalize splits a soft `f64` by `Rules::fits`; `Rules` should be declared in `lang` and
  generated; no target check yet that the convention's word matches `pointer_bits`.
- Backtraces: no call line per frame (native marks each call's origin; C cannot say where a
  call returns); no macOS (compact unwind, PAC). WASI reports the site only.
- Mach-O arm64 thread-locals are checked by linking with `ld64.lld`, never run on a Mac.
- Truly cold (empty store), threads wait ~1.1 s on the same `std` claims, once per toolchain.
- A cold run saves the shared file through one writer (~37 ms for 4.3 MB on `am3.wg`): values
  encoded, then copied into each enclosing blob (memo, query, body, file) and hashed whole.
- A shared file stays as its first writer left it: a toolchain's later projects compute again the
  library instances it lacks (a few ms each), and keep them in their own files.
- Warm builds emit every function again (`emit`, ~5 ms on `am3.wg` for `tour`): no code is cached
  per function by its LIR.
- A build after an edit that instantiates library generics parses those library files again
  (~18 ms for 31 files of `tour`): parse trees are traced, not saved.
- `vun-par` callers wait on uneven items (`wait for workers`, longest runs of 2 to 25 ms on
  `am3.wg`). Parse is ~11 MiB/s, the lexer ~145: its profile is flat now (the per-token decision
  over the frames and the specs positions take, rebuilt at each check).

## For the owner

- `~/.config/vun/toolchain.vun` needs a `linux.gnu.arm64` row (same compiler, sysroot and runner
  as `linux.arm64`) for glibc arm64 programs.

## Next steps, most useful first

0. Editor integration (the owner's current focus; Zed, Helix, VS Code, Neovim). Helix and
   Neovim highlight and run the server (347); VSCodium has the extension installed; Zed's
   extension builds for `wasm32-wasip2`; `cestef/tree-sitter-vun` is published (private until the
   language is public), pinned in `editors/tree-sitter/release.json`. Open: the owner looks at
   VSCodium and installs Zed's dev extension (`zed: install dev extension` on `editors/zed`);
   whether Zed lets the last pattern win is unverified; hover shows nothing on a declaration's own
   name or on a type annotation (`u64`, which typing leaves out); the TextMate grammar scopes
   `in(` as the loop keyword and types not at all; a doc comment on a record's field reads as a
   doc of an assignment in tree-sitter (`editors.vun` uses `//` inside records for now);
   `FromLiteral` could replace the hand walks of `Literal` in manifests, the lock file and the
   toolchain file. Inline assembly: `for` over a witness made per use (`Env::witness`,
   `Typer::iteration`) is not checked for conformance; an instruction value whose table holds
   types (`[0]type`) has no run-time layout, which only a library's report now hides.
1. Backends: Thumb agrees on every driver program (`linux.arm` through `qemu-arm` on `petal.wg`);
   `macos.arm64` links through both backends (`ld64.lld`, the SDK on `petal.wg` built from Zig's
   stubs) but runs nowhere here: a Mac, or a runner, would test it; riscv64 (`qemu-riscv64` is here, no
   sysroot); the stm32f4 board beyond the blinky (`bare_metal.vun` runs on qemu's STM32F405:
   gpio outputs, TIM2 waits, `std.task.run`): serial, systick, clocks past the reset oscillator.
2. Kernel runtime: riscv64 numbers; directories, seeking, environment.
3. Checks (268): a board and a WASI shadow, use after return; `fixed` kinds.
4. Corpus gaps (`cargo xtask corpus`, 3 planned): `std.meta.attr`, `std.Dyn`,
   `testing.Bench`/`Sim` and `mem.copy`. Lints: level `expect` shows as `warn` and never reports
   that a lint stayed silent; locals and parameters are not linted; the build's lint levels
   (item 5) do not reach `Lint::read` yet; the grammar generator still reads a PascalCase table
   name as "an enum" (`TokenKind` holds `#[lint.allow]`), which the table's shape could say.
   Vectors: comparisons to `Mask(n)`, vector instructions in the backends, `std.simd.dispatch`
   over `#[target.require]`. A trap in a `$` form inside a declaration's value is reported twice,
   by the form and by the value.
5. Plans (`vun build build.vun`): `flash`, `test_on`, dependency parameters, lint levels.
6. `std`: `task.Pool`, `testing.Sim`, `Str` and collections, generic bodies over `$A: Allocator`.
7. Evaluator: loans inside aggregate arguments, memory per nesting level. Packaging: signed
   caches, planned builds, `vun pkg why`. A comptime-heavy benchmark before perf audits.
8. Refactors: `vun-sema` itself is one dependency cycle (`env`, `eval`, `typeck`, `decl`,
   `member`, `layout`, `target`, `check`, `witness`, `asm`, about 12k lines): evaluation needs
   typed bodies and typing needs evaluation. Splitting further means owning traits in the lower
   part, as `Evaluator` does (`typeck` could reach `asm` through a registered checker). Also:
   `render/sarif`; `DiagId` and pass ids are strings; regalloc loads spilled operands where the
   form takes memory; LIR has no undefined value (padding copied as zeros); fuzz target `lir.rs`
   (written, never run).
