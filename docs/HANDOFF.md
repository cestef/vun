# Handoff

State of Vun as of 2026-10-02, for the next agent: what is open and what comes next. Nothing here
is durable; conventions and how-tos live in [`dev/`](dev/workflow.md), rules in
[`spec/`](spec/00-principles.md), history in `jj log` and [`decisions.md`](spec/decisions.md).
Read `CLAUDE.md` and [`dev/workflow.md`](dev/workflow.md) first.

## State

Everything is committed in the main chain. 813 tests pass; clippy,
`lines` and `layers` are clean; the corpus has 340 passing programs and 2 planned, and LIR
lowers every function the corpus reaches (3412 of 3412).

Built: the front end (a parser reading the productions of `nodes.vun`), packages and the store,
the persisted parallel query engine, sema, MIR and its checks, LIR with legalization, the C and
wasm backends, the native backend for x86-64, arm64 and Thumb-2, ELF, PE and Mach-O objects with
their readers (`vun-elf`, `vun-pe`) and the disassembler (`vun-disasm`) behind `vun debug disasm`,
the `gnu`, `mold` and `wild` linkers and lld's COFF flavor (`lld-link`), the toolchain file,
static Linux runtimes calling the kernel (threads, storage per thread, files, clocks) and the C
library's runtime, a Windows runtime (`std.os.windows`) calling `kernel32` itself, dynamic
library loading (`std.os.library`), the integers' byte and bit methods, `std.re` and pages over a
process's own memory (`std.os.pages`), backtraces,
lints (`lang.lint.Lint`, `std.lints.naming.case`), inline assembly as calls of `lang.arch`
declarations, editor packages generated from the grammar (tree-sitter, Helix, Neovim, Zed, VS
Code) and the language server. A function or type names an architecture's calling convention
(`abi.sysv`, `abi.win64`, `abi.cdecl`, `abi.stdcall`, `abi.fastcall`, `abi.thiscall`). Programs
run through either backend on linux.x86_64, linux.arm64, linux.arm (through `qemu`) and
wasi.wasm32, and through the C backend on windows.x86_64.msvc and windows.x86.msvc (under Wine).

Last landed: the lint levels of a build are configurable from the command line (`--lint
lint=level`, `LintLevelsOf`, scoped to the packages a command names; a name matching no lint or
group is `lint.unknown`, a level none of `lang.lint.Level`'s is `cli.invalid_value`), a lint at
`expect` warns when it does not report (`lint.unfulfilled`), a dependency's lint reports at most
`warn` while a dependent is built, and `#[lint.allow]` may be written on a local binding or an
expression as on a declaration (decisions 421 to 424). Before that: Windows is a target, two of
them (411 to 418). `windows.x86_64.msvc` and
`windows.x86.msvc` build through the C backend with clang for `x86_64-pc-windows-msvc` and
`i686-pc-windows-msvc`, linked by `lld-link` against a sysroot's import libraries, programs named
`.exe`; `std.os.windows` runs one thread with no C runtime, calling `kernel32` itself (handles
for file numbers, UTF-16 paths and command line, the performance counter and system time,
`__chkstk`, `_fltused`, and the string functions a C compiler assumes), and programs run under
Wine where the machine's tools file names it. A function or type carries its architecture's
calling convention (`abi.sysv`, `abi.win64`, `abi.cdecl`, `abi.stdcall`, `abi.fastcall`,
`abi.thiscall`), which the C backend writes as GNU attributes; Microsoft x64 is data
(`Rules::WIN64`), its scalar placements checked against clang, and the native backend does not
build it yet. A program loads libraries at run time (`System.load`, `std.os.library.Library`)
through `dlopen`, `dlsym` and `dlclose` or `LoadLibraryW`, `GetProcAddress` and `FreeLibrary`;
a Windows library is a DLL naming its exports. `vun debug disasm` lists an ELF or PE file's code
as instructions labelled by its symbols (`vun-disasm`, `vun-pe`), and `fuzz/` has the targets
`elf`, `pe` and `disasm`. A call of a function of `$` parameters alone folds by its value like a
primitive's (decision 407), and the integers gained byte and bit methods and `std.re` byte
patterns with `hexdump`. Before that: the compiler runs under WASI (389), and the playground runs
it in the browser.
`vun` builds for `wasm32-wasip1` (16 MB, 2.6 MB with brotli): the user's directories are
`vun_build::dirs::Dirs` (none there), a process is `vun_fs::PROCESS` (random there), and the CLI
leaves out `vun ffi` and mimalloc there. `site/play.html` runs `vun build --format json` in a worker through
`@bjorn3/browser_wasi_shim`, then the program, and draws the records as cards; its CodeMirror
editor (Helix's keys on request) is served by `vun util lsp` in a second worker, reading messages
inline where no thread starts (`vun_par::SPAWNS`). The page lays out a toolchain's tree in memory
(`toolchain/<build>/bin/vun`, found through the first argument, `vun_fs::EXECUTABLE`) and keeps
the memos the compiler saves between runs and in IndexedDB between visits: hello builds in
1.6 s cold and 0.1 s after; diagnostics show about 1 s after the page loads. Before
that: the wasm target links in process (388). `vun-link-wasm` is `wasi.wasm32.p1`'s linker
in place of `wasm-ld` (still `--linker lld`): strong over weak, COMDATs, unreached code and data
left out, the stack first in memory, `__wasm_call_ctors` and the bounds of named sections, weak
undefined functions as stubs that trap; `vun-wasm` holds the encoding and reads objects. The wasm
backend places data in sections, gives storage per thread as the program's on one thread, and calls
C variadic functions through clang's buffer. `tests/driver/wasm.rs` links the backend's objects with
clang's and runs both linkers' programs, which agree. Before that: literals identified by their instantiation (362) and property tests' gaps (363). A
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

Toolchains, targets and releases (364 to 367, the owner's model): a toolchain is a release
(`bin/vun` + `lib/`, found by `vun_driver::packages::Library`); targets are
`targets.<os>.<arch>.<abi>`; the machine's file is `tools.vun` (`VUN_TOOLS`); `vun target
list|show` and `vun toolchain show` show what builds and why (`vun_driver::setup`); `vun
toolchain list|install|remove|update` and a project's `.toolchain` install from releases and hand
off (`vun_driver::install`); `.github/workflows/release.yml`, `cargo xtask package|release` and
`install.sh` make and install releases (`docs/dev/commands.md`, "Releasing"); manifests are signed
and a pinned release's manifest hash is locked (368). Open there: the first tagged release
(`0.1.0`) has not run in CI yet; `install.sh` trusts its first toolchain as far as TLS.

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
(spread members of type bodies), 347 (editor queries last-wins, types captured, server fixes), 357 (test lenses,
`test.ignore`, one stream for the tests' program, checks' trap messages), 358 (calls through
borrows, a `$` parameter's constraint, `()` as a type, untyped locals reported where they are),
359 (typing's typed reasons, generic functions' own type; its type part approved) and 360 (reasons
in generic bodies, `W.Error` through the requirements), 361 (property tests, `lang.meta.params`,
`make`, `implements`, `f.call(t)` on callables, quiet threads), 362 (literals identified by
their instantiation), 363 (property generators, `test.gen`, `--seed` and `--cases`, tuples and
records in reflection's shape, floats in JSON), 380 (what a program reaches checked as its own
code, failing arguments as typed lines), 381 (a name in a pattern always binds;
`suspicious.shadowing` reports one hiding a constant), 382 (`lang.int.least` and `greatest`,
integers drawn by distance from 0 then side), 383 (generic bodies checked per instantiation,
not typed abstractly), 384 (benchmarks, `vun bench`), 386 (simulations; shrinking keeps only
smaller cases), 387 (coverage, `--cover`, `vun fuzz`) and 388 (the wasm linker), 398 (`{?}`, `{x}`, `{b}`, `Debug`), 399 (a name a spread selects is a
member), 400 (an array has its slice's impls), 401 (`impl I for T { }`), 402 (aliases for delegation; interface declarations are literals), 403 (a literal takes
its parameters from its annotation), 404 (an impl member takes them from its requirement) and 405 (a closure
argument takes them from the call), 406 (a call checks the bounds of its `$` parameters), 407 (a
call of a function of `$` parameters alone folds by its value; the integers' byte and bit
methods), 408 (`vun debug disasm` over ELF), 409 (a splice with no value at compile time runs
where it is spliced), 410 (a coroutine member's result from its requirement's interface), 411 to
413 (Microsoft x64 as data, `windows.x86_64.msvc`, PE disassembly), 414 (`std.re`), 415 (a DLL
naming its exports), 416 (`System.load`), 417 (calling-convention markers), 418
(`windows.x86.msvc`), 419 (lowering reaches every function the corpus reaches), 420 (the
readers' bounds, the command line's limits, a repeat's run-time length), 421 (the command line
sets lint levels), 422 (`expect` reports a lint that does not fire), 423 (a dependency's lint is
capped at `warn`) and 424 (`#[lint.allow]` on a local or an expression).
279 (static Linux targets), 282 (`&mut T` where `&T` is), 326
(section placements) and 345 (impls checked where their witness is made) were the owner's choice.

## Known gaps in landed work

- Verifying a stored build ahead (`Settle`, `vun-query`) guards a cycle of memos by the path it
  walks but records no memo found unsure, so a graph with a cycle is explored exponentially: an
  interface declared by a call (`Debug: lang.Interface = Rendering("debug")` under `#[lang.derive]`)
  made `Witnesses`, `Derives` and the typing of the call a cycle, and `vun fix` on any file spun
  for minutes (decision 402). Declare interfaces as literals; the engine should still not spin.
- An `impl` member may leave its parameters' types and its result to the requirement
  (decisions 403, 404), including a member of a witness a `#[lang.derive]` function builds and a
  result naming the witness's own member (`Result(usize, Error)`); a function written as an
  argument may leave them to the call (decision 405). Still written: a member with another
  number of parameters than its requirement (`Concat.concat` lacks `$R`, `Iterate` for a slice
  takes `&[T]` where the requirement says `Self`), a coroutine's result, and a parameter the call cannot say before the
  closure is typed (the accumulator of a `fold` nothing expects a result of).
- A `$`-generic C import (a pack inferred per call) is typed nowhere yet: such calls stay
  silently untyped (decisions 285, 286).
- Windows programs run only through the C backend, under Wine: the native backend builds neither
  Windows target (no COFF code path, no `Rules::WIN64` in `Rules::of`), and a DLL is checked by
  linking one, never by loading one built here.
- `vun debug disasm` reads a program's code but names no data; the ELF and PE readers are fuzzed
  (`elf`, `pe`, `disasm`) for bounds, and their agreement with llvm tools is checked by the
  driver's tests alone, not per instruction form.
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
- A new user's first session (`docs/reports/2026-10-01-new-user.md`) found: `a[..n]` checks
  clean and fails to build as `backend.unlowered`; a WASI stack overflow is a raw memory fault;
  `vun run -- file` and `vun test <name>` bind the word to `[path]`; `vun test -q` hides trapped
  tests; `std.panic.fail(..);` does not end its block; `naming.case` denies `Error := ();` in an
  impl; `vun test`/`build` skip a lint `vun check` denies. The report lists the diagnostics and
  missing pieces too.

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
- Properties: only shared borrows are lent (`&T`, `&[T]`), and a slice holds at most
  `std.testing.property.longest` (32) items.
- `vun fuzz` (decision 387) keeps its streams in memory only: no corpus in `fuzz/<test path>/`
  yet (the runner would need a way to read files the driver lists, or the driver to hand them
  over), and no traced comparisons, so a test of a byte among a long stream (a fuzz target's
  `&[u8]` of up to 32) is found as slowly as mutation reaches it. `vun cover` does not exist.
- Simulations (decision 386) have a virtual clock and a seeded scheduler over a tuple of tasks;
  not yet a simulated network or disk, nor choices drawn by the tasks themselves (a task would
  need the `Sim` its test holds mutably while `run` does).
- Benchmarks (decision 384) report one measurement: no statistics over samples, no
  `--measure instructions`, no baselines in `.vun/bench/` nor `--fail-over`.
- A `$for` over a `.{ }` literal does not lower ("not supported in compiled code yet"); over an
  array it does.
- A pattern-only form where a value is expected is never reported: `&xs[..]` parses its
  subscript as a `Rest` (patterns share the expression grammar), HIR lowering makes it
  `ExprKind::Error` through the `_` arm of `Lower::expr_kind` with no diagnostic (it has no
  `DiagCx`), and typing then calls it a compiler bug. Either the grammar's operand contexts keep
  `Rest` to patterns, or lowering reports what it turns into `Error` without a parse error.
- `#[test.fuel(n)]` means something only where a mode counts steps (`interp`, not built).
- Loose files stay in no namespace (`test.no_namespace`): a namespace is a package's, and the
  corpus and every example live in one. Recommended to keep; the owner may overrule.
- Where the system runs one thread (WASI), the first trap ends the run.

## Next steps, most useful first

The website: `site/` (Vite, TypeScript) holds the landing page (the hero, the brand's motion
ported from `assets/motion`, the install command) and the playground; the spec reader was
removed at the owner's request; the playground now carries a guided tour in lessons
(`site/lessons/`, checked by `pnpm check:lessons`). `.github/workflows/site.yml` deploys it to
GitHub Pages when `main` changes what it is made of (a pull request only builds); a host must
still send the headers of `site/public/_headers`, or the editor has no language server. Open in
the playground: a program that never ends runs
until stopped (the stop button ends the worker), and threads, which WASI lacks, are not
offered. The wasm backend still refuses inline assembly (no `lang.arch.wasm32`
table), naked functions and addresses past a function's start, and reports no frame sizes or
debug information.

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
   Windows: `windows.x86_64.msvc` and `windows.x86.msvc` run through the C backend under Wine;
   the native backend builds neither and needs a COFF code path and `Rules::WIN64` in `Rules::of`.
2. Kernel runtime: riscv64 numbers; directories, seeking, environment.
3. Checks (268): a board and a WASI shadow, use after return; `fixed` kinds.
4. Corpus gaps (`cargo xtask corpus`, 2 planned): `std.Dyn` (`corpus/planned/types/dynamic.vun`,
   plus the `Encoder` `meta/derive.vun` waits for) and `mem.copy` (`corpus/planned/testing/tests.vun`,
   with the `node`, `Packet`, `crc32` stand-ins). Lints: the grammar generator still reads a PascalCase table
   name as "an enum" (`TokenKind` holds `#[lint.allow]`), which the table's shape could say.
   Vectors: comparisons to `Mask(n)`, vector instructions in the backends, `std.simd.dispatch`
   over `#[target.require]`. A trap in a `$` form inside a declaration's value is reported twice,
   by the form and by the value. A function evaluated while a `$if` step is typed cannot pass
   one of its run-time locals to a `$` parameter (`same(type, t, A)` inside a `for`:
   `comptime.unsupported`), though the same call evaluates at compile time elsewhere.
5. Plans (`vun build` on a package): `flash`, `test_on`, dependency parameters.
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
