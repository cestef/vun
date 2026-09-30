# Handoff

State of Vun as of 2026-09-30, for the next agent: what is open and what comes next. Nothing here
is durable; conventions and how-tos live in [`dev/`](dev/workflow.md), rules in
[`spec/`](spec/00-principles.md), history in `jj log` and [`decisions.md`](spec/decisions.md).
Read `CLAUDE.md` and [`dev/workflow.md`](dev/workflow.md) first.

## State

Everything is committed in the main chain; no `jj` workspace holds work. 689 tests pass; clippy,
`lines` and `layers` are clean; the corpus has 312 passing programs and 2 planned, and LIR
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

Last landed: literals identified by their instantiation (362) and property tests' gaps (363). A
function literal, closure or type literal in a body with `$` parameters and run-time ones is
evaluated under the body's instantiation (`Func::within` and a nominal type's identity hold an
`Instantiation`; a compile-time call is the one binding every parameter), so `() -> u16 { n + n }`
in `($n: u8) -> u16` builds and `struct { v: [n]u8 }` there is a type per `n`; the test runner's
case is a closure again. Properties now generate floats (their encoding one choice, JSON as the
exact hexadecimal form), arrays, tuples and records (reflection's one shape covers both), and
borrowing parameters, lent what the case holds (`Lent` on `lang.mem.Ref`, `Store` for `[T]`, a
prefix of up to 32 items), so fuzz targets (`&[u8]`) run as properties; `#[test.gen(g)]` picks a
parameter's generator (attributes are typed before they are evaluated); `vun test --seed n
--cases n` (`std.testing.Options`, `vun_command::Values::Count`). On the way: `Int(r).parse`, and a
MIR temporary holding an expression's value takes its type (`d < "0"[0]` did not build). Before
that, property tests (361). A test with parameters runs over 100 generated cases, each on
its own thread; values come from `std.testing.gen` (a type's `Arbitrary`, or reflection through
`lang.meta.make`) reading a recorded choice stream, and the first failing case is shrunk on its
choices, reported once with its arguments as JSON (`(x: u8) { assert(x < 200); }` reports
`x = 200`). On the way: `lang.meta.params`, `make`/`Parts`, `implements`; `f.call(t)` on functions
and closures; `$` locals run nothing; `std.os.thread.reporting`. Before that, typing says why in
generic bodies too (360). An unbound `$` parameter, and a
requirement of an interface made for a constrained one (`W.Error`), have no value for the reason
`Unevaluated::Unbound`, and a form giving up for it is left to the instantiations; reasons travel
through locals bound without a type and through expected types a context cannot know; `W.Error`
is typed and evaluated through the interface's requirements (`Env::required`); the verifier covers
plain generic bodies (`Program::bodies`) and logs name places as `path:line:column`
(`OriginId::located`). Before that, typing says why (359). Typing records the form where each silent gap starts
with a typed reason (`Found::gave_up`, `GaveUp`), logged under `vun_sema=debug`; a body lowering
stops in over one is reported at that form with the reason (`Unready::explained`, the error
naming the form and why rather than a local), `vun debug dump types` marks each, and
`mir::typing_gives_up_nowhere_a_corpus_program_reaches` is typing's verifier on real inputs.
The gaps it found were closed as rules: a function with `$` parameters named as a value is of
its own type (`TyKind::Generic`, accepted where a function type is), a name whose declaration has
no type here takes its value's (`closed` in `std.fmt.Writer`), any type-valued expression names
its type's members (`Thread(R, F).spawn`), `*T` and `?T` are types, `defer e` is `()`, `_` takes
its context's type, and a `lang.Range` value is typed. Before that, calls and `$` arguments (358). `h()` calls what a borrow `h` holds; a `$`
parameter implements its constraint, so `R` of `$F: Call((), R)` is inferred in a generic body;
`()` where a type is expected is the unit type however it is reached; a body with an untyped
local is reported at the local, with typing's own error, or as a compiler bug naming `vun debug
dump mir` when typing reported none (`corpus/accept/types/callables.vun`). Before that, test
lenses and the runner's gaps (357). The server offers a `Test` lens over each
test a file declares (`Session::tests`, from `std.meta.collect` through `vun_sema::testing::Tests`)
running `vun.test`; `#[test.ignore("why")]` reports a test `ignored`; `vun test` reads the
program's output and error as one stream, so a test's output and backtrace come with its line
(`vun_driver::testing::Run`, shared with the server), and when a run ends the server titles
each lens with the outcome and shows a trap as an error where it fired; run-time checks name their trap site with
their `Reason`'s message. The nushell completion script exports its root command as `main`, as a
module named `vun` must (`use vun.nu *`), and `cargo gen` keeps what Zed writes building the dev
extension (`extension.wasm`, `grammars/`), which it had removed as stale. Before that, tests run (354, 355). A trap ends its thread and `Thread.join` gives
`Result(R, lang.panic.Trap)`; `std.testing.runner(ns)` runs each unit test on a thread of its own
and writes JSON lines (`std.fmt.json`, derived by reflection); `vun test [path] [names]...` builds
it for the namespace of a file, a directory or a package (`Roots::Members`, `vun_sema::testing::Runner`)
and reports each test. On the way: a pointer into a cell reads a run-time place, so `assert` over a
global `Atomic` no longer fails to build; a failed assertion's site is its condition; `vun-data`
derives tagged enums. Before that, the performance pass (348). Criterion benchmarks (`cargo bench -p
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
(spread members of type bodies), 347 (editor queries last-wins, types captured, server fixes), 356 (an optional path, the nearest
`build.vun` when none, for every command; only `vun test` follows it yet), 357 (test lenses,
`test.ignore`, one stream for the tests' program, checks' trap messages), 358 (calls through
borrows, a `$` parameter's constraint, `()` as a type, untyped locals reported where they are),
359 (typing's typed reasons, generic functions' own type; its type part approved) and 360 (reasons
in generic bodies, `W.Error` through the requirements), 361 (property tests, `lang.meta.params`,
`make`, `implements`, `f.call(t)` on callables, quiet threads), 362 (literals identified by
their instantiation) and 363 (property generators, `test.gen`, `--seed` and `--cases`, tuples and
records in reflection's shape, floats in JSON). 279 (static Linux targets), 282 (`&mut T` where `&T` is), 326
(section placements) and 345 (impls checked where their witness is made) were the owner's choice.

## Known gaps in landed work

- `R` of `$F: lang.ops.Call((), R)` is inferred from functions and closures, not from a struct's
  `Call` impl: a call passing a struct that implements `Call` writes its `$` arguments.
- A declaration destructuring into `mut` binders, `(mut a, mut b) := (false, false);`, is typed
  but not lowered ("not supported in compiled code yet"); `std.testing.Options.chosen` declares
  them one by one.

- A generic body is typed only as far as its `$` values are not needed: a local of an
  unconstrained `$T: type`, and a method whose signature names a requirement (`w.write_all(..)`
  returning `Result((), W.Error)`), stay untyped there and are checked per instantiation. Typing
  them abstractly would need an opaque type for `T` and for a requirement's value (an associated
  type), with every compile-time evaluation reading one deferring rather than trapping: a
  language decision for the owner.

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

## Running tests: what is left

Runs natively (354, 355, 357, 361); spec 11 is the reference. Open:
- Properties: the language server's lens shows `falsified` and the trap, not the arguments, and
  runs with the default seed and count. An integer's choice is any `u64` wrapped into its range
  (`lang` gives no range's bounds to draw within), so shrinking a wide integer walks through
  values outside it; a float's is bounded by its width. Only shared borrows are lent (`&T`,
  `&[T]`), and a slice holds at most `std.testing.property.longest` (32) items.
- Fuzz targets (`&[u8]`) run as properties under `vun test`; the coverage-guided fuzzer (`vun
  fuzz`) and the corpus in `fuzz/<test path>/` do not exist. `Bench` and `Sim` do not exist:
  the runner treats every test with parameters as a property.
- `#[test.fuel(n)]` means something only where a mode counts steps (`interp`, not built).
- Loose files stay in no namespace (`test.no_namespace`): a namespace is a package's, and the
  corpus and every example live in one. Recommended to keep; the owner may overrule.
- Where the system runs one thread (WASI), the first trap ends the run.
- `persist::an_unchanged_program_built_again_computes_nothing_it_saves` failed twice in full
  driver runs from an empty `target/tmp/memos` on `petal.wg` (`VUN_QUERY_CHECK=1`), then passed
  on a third; alone, or with the `testing` or `runtime` tests, it passes. On 2026-09-30 it failed
  in 2 of 4 full memo-checked runs on `petal.wg` and passed 5 of 5 alone. Shared entries are
  content-addressed, so a rewritten entry is not the cause; not found yet. It reproduces: after a
  full `VUN_QUERY_CHECK=1` driver run, the test alone fails every time with the memos that run
  left in `target/tmp/memos` (4 of 4 on `petal.wg`, on 361 and on 363 alike), and passes every
  time once they are removed. It passed with `VUN_LOG=vun_query=debug` on, which slows it.

## Next steps, most useful first

The owner's queue, in order: the editor polish of item 0 (hover on
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
