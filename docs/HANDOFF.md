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

- A pattern naming a value binds a new local of that name: `.ok(true)` binds `true` and matches
  everything (found by `style.unused` in `corpus/accept/programs/files.vun`, now `.ok(.true)`).
  Shadowing a prelude name is allowed, so nothing reports it; a pattern whose name resolves to a
  value should compare with it, or be refused (a language decision for the owner).

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

## Settled direction: pre-evaluated output

Discussed with the owner: no standalone "pre-compile, then compile from that" command.
- *As a cache:* the saved query memos (beside a root file, and the shared library memos) already
  keep what compiling computed, keyed by content. What is missing is below them, a code cache;
  finishing it gives this for free.
- *As residual source, now:* a view `vun debug dump expanded`, a row of `vun_driver::view::View`
  (so editors offer it too): the program with each `$` form evaluated, `$for` and `$if` expanded,
  generated types spelled out. A view only, never a build input: emitted text loses origin chains.
- *As a distributable artifact, later:* prebuilt package entries in the shared store (the Nix
  direction), keyed by content and target. Compile-time values read the target (`TargetOf`), so
  entries are per target; only instantiations the package itself reaches can ship, generic code
  ships as HIR and types; `embed`, the build plan and the toolchain version go into the key;
  origins survive so diagnostics and debug info still point at source.

## Proposed direction: running tests (confirm with the owner first)

The owner turned down three designs as hacky: a process per test, a trap handler that jumps
back (setjmp and longjmp), and running tests only at compile time. The direction below adds no
test machinery. Test isolation falls out of one general rule the spec already implies. Present it
to the owner, write it as a proposed decision, then build it in this order.

1. **A trap ends its thread, and joining reports it.** Spec 07, section 3 already says a trap
   "ends the current execution context" and never defines the context. Define it as the thread
   (a task where `std.task` schedules tasks). A trap ends that thread alone, and
   `std.os.thread.Thread.join` gives how it ended: returned with the value, or trapped with the
   `lang.panic.Trap` (its site id, which the trace table maps to a message and origin). A trap in
   the thread the program started in ends the program, as now. There is still no unwinding:
   `defer` does not run, and what the thread held stays held, which the checks of unsafe code can
   report. The handler each runtime exports (`lang.panic.symbols.trap`: `std.os.libc`,
   `std.os.linux.kernel`, `std.os.wasi`) writes the backtrace as now. In a spawned thread it then
   stores the trap in the thread's record (`std.os.thread.Placed.record`, which the runtime
   already keeps and `join` copies back) and exits the thread alone: `pthread_exit`, the Linux
   `exit` syscall rather than `exit_group` (the joiner's futex on the child tid wakes as now), or
   WASI's thread exit. Where there is one execution context (bare targets without `std.task`),
   the program ends. At compile time a trap already ends the call being evaluated: it is the same
   rule. The rule is general, not for tests: supervisors, servers and `std.task.Pool` gain it too.
2. **`std.testing.runner(pkg)` is library code (spec 11, section 1).** Its tests are
   `std.meta.collect(pkg, std.testing.test)` (decision 353); each test's value is
   `c.ns.$(c.decl.name)`. The runner is an ordinary program: it runs each test on its own thread,
   several at once, joins them and writes one JSON line per test (name, origin, returned or
   trapped with the site's message), then a summary, and exits with a failure status when any
   test trapped. Unit tests, those with no parameters, come first. The other kinds of spec 11
   (property, fuzz, `Bench`, `Sim`) are chosen by the test's parameter types, read through
   `lang.meta`. `#[test.ignore(...)]` and `#[test.fuel(n)]` are read with `std.meta.attr`. A
   filter (test names) comes in as program arguments.
3. **`vun test [file]`** builds a program whose entry is the runner over the package of `file`,
   or over the file's own namespace for a loose file. Open: how that root is named without
   writing source text. Two candidates: the plan's `std.build.Options.output` gains `.tests`
   (the program built from the package's root, with the runner as its entry), or the driver adds
   a generated root through `vun_syntax::make`, typed and never a string,
   `start := std.testing.program(<package>);`, beside `std.os.start`. The same JSON stream is
   what every mode writes (spec 11, section 3): native first; qemu and the interpreter later.
4. **Editors.** `lang.meta.Decl` gains `origin` (where the member is declared). The driver lists
   a file's tests by evaluating the same `collect` through the query engine (`Session::tests`),
   so the server's test lenses (`vun.test`, beside `vun.run`) come from the library's own
   definition of a test, never from matching `#[test]` in the text. Results come back from the
   JSON lines and show as the lenses' titles and as diagnostics at the trap's origin.

Known trap on the way: a function evaluated while a `$if` step is typed cannot pass one of its
run-time locals to a `$` parameter (see item 4 of the next steps). The runner should read
attributes through `$` descriptors, as `std.meta.attr` does.

## Next steps, most useful first

The owner's queue, in order: the test runner above, then the editor polish of item 0 (hover on
a declaration's own name and on type annotations, stopping a run, views of the function at the
cursor, VS Code commands), then the rest of lint levels in item 4 (command-line levels,
dependencies capped at `warn`, `expect`, `#[lint.allow]` on locals).

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
   Richer server (349): views as `source` code actions (`vun.view`, shown by `window/showDocument`),
   hovers with values and layouts, type-only inlays, a run lens (`vun.run`). Open: test lenses (no
   `vun test` or `std.testing.runner` exists yet); a run cannot be stopped from the editor, nor
   given input; views are of the whole file, not the function at the cursor;
   per-editor polish (VS Code commands, Zed has no code lenses); untried in a real editor.
1. Backends: Thumb agrees on every driver program (`linux.arm` through `qemu-arm` on `petal.wg`);
   `macos.arm64` links through both backends (`ld64.lld`, the SDK on `petal.wg` built from Zig's
   stubs) but runs nowhere here: a Mac, or a runner, would test it; riscv64 (`qemu-riscv64` is here, no
   sysroot); the stm32f4 board beyond the blinky (`bare_metal.vun` runs on qemu's STM32F405:
   gpio outputs, TIM2 waits, `std.task.run`): serial, systick, clocks past the reset oscillator.
2. Kernel runtime: riscv64 numbers; directories, seeking, environment.
3. Checks (268): a board and a WASI shadow, use after return; `fixed` kinds.
4. Corpus gaps (`cargo xtask corpus`, 3 planned): `std.Dyn` (`meta/derive.vun` waits for it and
   an `Encoder`), `std.meta.decls` of a struct type (namespaces only for now),
   `testing.Bench`/`Sim` and `mem.copy`. Lints: level `expect` shows as `warn` and never reports
   that a lint stayed silent; the command line sets no levels yet, and dependencies' lints are
   not capped at `warn` when building a dependent (the build's levels apply to its own
   package); `#[lint.allow]` applies to declarations, not to locals; the grammar generator still reads a PascalCase table
   name as "an enum" (`TokenKind` holds `#[lint.allow]`), which the table's shape could say.
   Vectors: comparisons to `Mask(n)`, vector instructions in the backends, `std.simd.dispatch`
   over `#[target.require]`. A trap in a `$` form inside a declaration's value is reported twice,
   by the form and by the value. A function evaluated while a `$if` step is typed cannot pass
   one of its run-time locals to a `$` parameter (`same(type, t, A)` inside a `for`:
   `comptime.unsupported`), though the same call evaluates at compile time elsewhere.
5. Plans (`vun build build.vun`): `flash`, `test_on`, dependency parameters.
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
