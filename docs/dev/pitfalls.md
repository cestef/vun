# Pitfalls

Traps that cost a session before. Add one when it costs you time.

## Origins and queries

- Origins are equal by identity: lowering that makes two entities from one piece of syntax gives
  each its own origin (`Lower::own`); `Hir::verify` catches it. `OriginId` has no order.
- Persisted origins are pooled by identity, so two origins of one identity are read back as one.
  A declaration's identity counts the declarations of its name before it in the same scope
  (`Anchor::Declaration::nth`), and `OriginTable::declaration` refuses two of one identity in one
  text. Without that, two locals `pointer` in sibling `match` arms made a reused `mir` memo differ
  from its recomputation only under `VUN_QUERY_CHECK=1`.
- A query key must equal another only when computing either gives the same value: a key equal by
  origin alone kept the index a node had before an edit, so a saved memo was computed at the
  wrong node (decisions 195 and 283). A key carrying anything `compute` reads must compare it,
  and every `Hash` agrees with its `Eq`. `VUN_QUERY_CHECK=1` catches a violation by name.
- Query cycles (decision 198): a value computed from a `cycle` value is provisional and never
  memoized, only the query reached again keeps its memo; verification treats a memo reached
  again through its reads as `Why::Cycle`. Programs with cycles (recursive types through
  `kinds`) still re-save their queries on every warm run: the recomputed memos' reads differ in
  order from the saved ones. Cheap, but a fixpoint-style cycle (salsa's) would remove it.
- `vun_par::Map::get_or_insert_with` (papaya) runs its closure in every thread that races to
  add a key, then keeps one value: `Table::slot` pushes an entry for each, so a table holds slots
  no key maps to. Walk a table with `Table::entries`, which skips them; one saved with its key
  gave a second, memo-less copy that a later build found first.
- Process-wide tables (`SourceFiles::global()`, `OriginTable::global()`, interners) are shared by
  every session in the process.
- Persisted queries need unique `Query::NAME`s and codecs; set queries are `INPUT`. Query values
  hash by content (`BTreeMap`); reports are data.
- Saved queries are stamped with the compiler build that wrote them; another build's are refused
  and recomputed (decision 241), so a changed query type never decodes into garbage.
- Nothing has a target until the driver chooses one; code outside the driver sets `TargetOf`.

## Building and generating

- `unset CARGO_TARGET_DIR` if the shell inherits one: builds must go to the repository's
  `target/` (a second `jj` workspace needs its own). `cargo clean` after moving the repository.
- `cargo gen` never depends on what it generates: change the source, run it, fix consumers.
  Generated Vun is built with `vun_syntax::make`, never formatted as text.
- `cargo fmt` rewraps code: edit scripts must assert their replacements applied.
- `fuzz` is a workspace member (workspace lints, one lockfile); `cargo +nightly fuzz` still runs
  from `fuzz/` and builds into the root `target/`.
- The machine's tools file (`tools.vun`) is read by every session that chooses a target, tests included (see
  `commands.md`). Its field errors reuse the `manifest.*` ids, as the lock file's do.

## Tests

- Snapshots are written only with `VUN_BLESS=1` (`0` or empty is off), and stale ones fail.
- Before committing sema or `std` changes, run `cargo nextest run -p vun-driver`: the corpus does
  not exercise `std.fmt.print`'s staged paths.
- The driver tests share the libraries' memos in `target/tmp/memos`, keyed by the test binary,
  not by the library sources: after editing `lib/` without the binary changing, sessions compute
  the libraries again and never rewrite the shared file, so
  `persist::an_unchanged_program_built_again_computes_nothing_it_saves` fails (on a remote mirror
  too). Remove `target/tmp/memos`.
- `BackendCaps` is a `Feature` set; unimplemented backends are `vun_backend::Planned`;
  `Store::stage` returns a per-process `Staging` guard removed on drop; `xtask`, `vun-command`
  and `generate` declare tool-local diagnostic ids.
- A program run through a runner is read through files, `vun_fs::Captured`, not `Command::output`:
  `wine64` leaves a `wineserver` that holds the pipes for about 8 seconds, so a run through pipes
  lasts as long as the daemon, in the tests and in the run times `vun debug compare` reports.

## Sema

- Checks report only what they know: unknown types and `Unevaluated` values are silence.
- Typing checks both branches of an `if` against the expected type, even when the condition is
  known: `x: Int(0..=0) = if true { 0 } else { 1 }` is `int.does_not_fit`.
- The typer evaluates only through `Typer::evaluating()` (what typing found so far, shared) and
  `Typer::known_value` (annotations and `$` arguments, typed first when evaluating needs it),
  never through the bare `Typer::env()`: without what typing found, a call whose `$` arguments
  are inferred (`TypeOf(x)`) or a form typed by its context has no value, and the call stays
  silently untyped. `VUN_LOG=vun_sema::typeck=trace` shows each evaluation that gave no value.
- A `None` from a typing function with no report is a silent give-up. `Typer::expr` records the
  form where it starts (`Found::gave_up`, why as a `GaveUp`): a form left untyped with nothing
  under it reported or given up on. Where a lookup giving the form's own type returns an
  `Unevaluated`, pass it through `Typer::known` rather than `.ok()`, and record any other known
  reason with `Typer::give_up`, so the reason is kept and logged. Lowering reports a body stopped over
  such a form at the form, with the reason; `vun debug dump types <file>` marks each (`untyped,
  gave up on Call: ...`), and `mir::typing_gives_up_nowhere_a_corpus_program_reaches` fails on
  any silent one in a body a corpus program reaches, the plain bodies of generic functions
  included. A reason must travel to what depends on the form: a binding site gives each binder
  its type or why (`pattern` takes `Result<Ty, GaveUp>`, `Typer::matched` finds why a value has
  none), and a rule passing its expected type to a part passes why it has none too
  (`Typer::expected_of`, `expecting`); forgetting either turns a deferred or reported gap into a
  silent `Form` one the verifier flags. In a plain generic body, `Unevaluated::Unbound` is the
  expected reason: a value only an instantiation binds.
- `($for x in xs { f(x) })` is a parenthesized `$for`, of type `()`, not a tuple: a tuple literal
  generated by `$for` needs a trailing comma, `($for x in xs { f(x) },)`, as `(x,)` does. An array
  needs none: `[$for i in 0..n { a[i] }]`.
- Code evaluated outside a function body (attributes, a declaration's value) has no typed body
  to read inferred `$` arguments from: type it first (`Typer::typed`, then `Env::typed_by` with
  what it found), as `Env::values` does for attributes. Plain `eval` of `test.gen(digit)` fails,
  and a caller dropping failures (`filter_map(.. .ok())`) hides it.
- A range index of an array field, `held.items[0..n]`, is the run `[T]`, which has no size: borrow
  it (`&held.items[0..n]`). Only indexing through a borrow reborrows the run itself.
- A `match` or `if .some(x)` binder named as a sibling member (`option` in `Options`) shadows it,
  and the library checks refuse `names.shadowed`.
- `VUN_LOG` shows only what is computed: queries reused from saved memos log nothing. Log a fresh
  run with `VUN_STORE=<empty dir>` and the `.vun/` beside the root file removed.
- The parser's depth bound counts each operand being read and each link of a chain; past it the
  rest of the expression is one `Error` node, so later stages never recurse deeper than about
  twice `MAX_NESTING`.
- The parser is the productions of `nodes.vun`: a syntax change is a grammar change (and
  `cargo gen`), never parser code. A table the generator cannot type from its names (all of
  them both token and node kinds, as `[.\for, .\if]`) is typed by its use in `.of`; a table named
  as a kind's production constant would be (`staged` for the kind `staged`) clashes in Rust.
- The generator links `vun-syntax`: code naming a node kind or field `cargo gen` has not yet
  written breaks the run; write the generated item by hand first, then regenerate.

## Writing Vun

- `or` is a keyword; child namespaces are not in scope by bare name
  inside their parent; private fields are visible only inside their struct literal; kind
  attributes go on the type's literal; inside a `lang` namespace file its own names shadow
  `lang`'s; integer arithmetic is exact (`x * 2` for `u32` does not fit `u32`); no operator may
  be written inside `lang`'s operator impls; attributes are bare only from the prelude's
  namespaces.
- A borrow of an array sliced, `y[i..j]` for `y: &mut [N]T`, is already a borrow of the run
  (`&mut [T]`): `v.store(y[i..j])`, not `&mut y[i..j]`. An integer literal is never a float:
  `V.splat(0.0)`.
- Things the std batches kept tripping on: a binding is `mut x := e` or `mut x: T = e`, never
  `mut x = e`; `mut n := 0` has the range `0..=0`, so `n += 1` traps (write `mut n: u32 = 0`);
  a variable shift `1 << n` has no width (`one: u64 = 1; one << n`); a bit operation needs both
  operands `uN` (`(a - 1).wrap(u64)` first); an array type is `[n]T`, not `[T; n]`; a slice that
  is returned from a `&mut self` method keeps the borrow (return offsets, slice a copy of
  `self._text`); a closure that captures by `[&x]` sees `x` as it is, not as `&x`; `for x in
  array` gives the items, `for x in slice` borrows them; `or`, `if let` and `|>` do not exist
  (`if .some(x) = e` does); a string literal is an array of its length, so comparing two of
  different lengths needs `&[u8]` parameters.
- A table in `desugar.vun` naming `lang` items is a tuple `.{ }`: its items have types of their
  own, and an array would not type. Borrow modes are lists of `BorrowWord` (`grammar.vun`).
- A `Code(T)` parameter takes its argument as written, typed as a `T`: `lang.arch.asm({ ... })`,
  `assert(x > 0)`. A quote `#{ }` there is a `Code(T)` where a `T` is expected (`types.mismatch`).
- A block of instructions in a generic body whose calls depend on a `$` parameter is untyped
  there, and read against its table where the function is instantiated.

## Compile time

- A variant written bare inside a non-empty array that is the default of a struct field does not
  build (`plan.stopped`: "a function whose body does not build"); name it as a typed constant
  and put that in the array (`std.build.uf2_drive`).

- A `mut` local declared without a type takes its first value's range, and an update narrows
  back into it: `mut j := i` for `i in 1..n` is `Int(1..)`, and `j -= 1` traps when it reaches 0.
  Write `mut j: usize = i`. Conditions do not narrow a mutable place into the loop's body either
  (`for j > 0 { xs[j - 1] }` is fine, `f(j - 1)` for a `usize` parameter is not).
- Integer arithmetic is exact, and so are declarations without a type: `first := 0x4002_0000`
  keeps its value's range, where `first: usize = ...` has all of `usize`'s, and adding to it can
  pass the width. A shift of a constant by a run-time amount (`1 << n`, `n: u32`) has no width;
  narrow the amount (`Int(0..32)`) or bring the constant into a `uN` first.
- A device's registers are a `#[layout.c]` struct of `std.hw.Reg` fields, borrowed at their
  address with `unsafe std.hw.at(T, address)` wherever used; a token stores what places them
  (a port's index), not the borrow.

- A pattern test is written pattern first, `if .some(h) = x`. Reversed, `h` leads nowhere; in a
  dependency's compile-time function that surfaces as `comptime.export` at the value exported,
  labeled where evaluation stopped, with the names errors of that file.

## Native backend

- An instruction's implicit registers belong in its form (`.implicit`), or a snippet's scratch
  may land in them.
- A terminator's operands are read after `Cx::leave`, which counts their uses but frees
  registers only in `Cx::left`.
- A snippet step has at most 4 operands (`OPERANDS` in the encoder). A result is placed after
  its step's other operands, so taking a dying operand's register never unbinds one the step
  still reads.
- An instruction patched when the function ends is re-emitted into a listed buffer and swapped in
  with `Buffer::replace`, so the listing shows the final value (arm64's frame size; x86-64 still
  overwrites its immediate and lists the placeholder).
- arm64: homes past 256 bytes below `x29` take an index built in `x15` (`FAR`); `x16`/`x17` are
  the framework's carrier and temp, so nothing else may use them across a jump's moves.
- A LIR value never spans registers (no `Loc::Pair`): wide values are split by legalization.
- Arm ELF is `REL`: GNU ld reads a relocation's addend from the field, never the entry, and a
  section symbol would take the offset as an addend. Thumb objects name the symbol itself and
  hold addends in fields (`Machine::implicit_addends`, `bind_locally` skips them).

## Boards without a console

- Never call a ROM routine before reading its table pointer back: it is at `0x16` on Arm, not
  `0x18`.
- A register is a value: `r := self.regs.control` copies it and `r.write` hits the copy. Take
  `&self.regs.control`. Check a struct of registers against the datasheet's offsets (a missing
  reserved word shifted every register after it).
- `vun flash` loads the `.uf2` beside the program; `picotool load` of the extensionless
  `.vun/build/app` itself needs `-t elf`. Either races a reboot: retry, or `picotool reboot -f -u`.
- SRAM does not survive the ROM's reboot into BOOTSEL: filled with their own addresses, every
  word of `0x20001000` to `0x2007f000` and `0x20080000` to `0x20082000` read back zero
  (`probes/sram`). The watchdog scratch registers survive, but `picotool save -r` refuses them
  (`Save range crosses unmapped memory`). What `picotool info -a` prints are the reboot's two
  parameters, as "reboot param 0" and "reboot param 1" (`probes/param`): `rom.reboot`'s `p0` and
  `p1`, 32 bits each, except that bits 0 and 1 of `p0` hide BOOTSEL's interfaces. A program with
  no console can leave a record there.
- The USB controller needs a system clock of at least 48 MHz: from the crystal's 12 MHz the host
  sees the device and resets it, and its first requests are never answered (`device descriptor
  read/64, error -71`). `rp2350.usb.clock.run` starts the system PLL (150 MHz) and keeps the
  peripheral clock on the crystal.
- Debugging with no console: `probes/usb` and `probes/param` show the pattern, a program that
  reboots into BOOTSEL with what it saw as the parameters, and an early `rom.reboot(.., 90_000,
  ..)` that returns the board by itself if the program hangs.

## Editors

- Queries are written general patterns first: Helix since 25.07, Neovim and Zed let the last
  pattern matching a node win, so a catch-all such as `(name) @variable` goes first.
- A generator's output directory may hold another tool's files (`node_modules` after
  `npm install` in `editors/vscode`, with links); what the generator `keeps` is never walked.

## Assets and remote runs

- Motion Canvas lays out every `Layout` under a layout, reaching through plain `Node`s: a `Path`
  (a `Layout`) inside a `Node` inside a layout box is placed by the flexbox, not by its
  transform. A shape drawn at a place it computes sets `layout={false}` (`Mark`).
- Motion Canvas `Txt` in a layout collapses spaces, and `textWrap="pre"` breaks nested `Txt`
  pieces onto lines of their own: terminal text keeps its spaces as no-break spaces.
- An `Img`'s natural size is known only once loaded: `yield img.toPromise()` before reading it.
- `cargo xtask remote` passes its words through ssh unquoted: `sh -c "..."` and nextest filter
  expressions with parentheses lose their quoting. Filter by name (`cargo nextest run -p crate
  name`). A snapshot blessed on a host is overwritten by the next sync: bless from a command
  that writes locally, or print the result and copy it back.
- The same holds for `cargo gen` on a host: the next `rsync --delete` of the working copy puts
  back the old generated files, and a build there fails on what they lack. Copy every generated
  directory back (`rsync host:<dir>/ <dir>/`) right after each `cargo gen`.
- A `vun` older than the library it runs against reads a package of the library that uses newer
  manifest fields and reports each as `manifest.unknown_field`. A toolchain with `.requires`
  (442) says it is too old instead; one without cannot, so update the toolchain first when a
  library manifest "has unknown fields" nobody misspelled.
- What a compile-time call costs shows in `VUN_LOG=vun_query::cx=debug,vun_sema=debug`: each
  `computing query="metered_call"` line carries the `call{ f="name" }` spans of the calls it is
  inside, outermost first. Count by the first span to find what starts the evaluation (a
  declaration's value, a lint, an `assert` condition) and by the last for what is called most.
  Code spliced by a `Code(T)` parameter is evaluated at compile time when it has a value, so a
  constant condition runs in the interpreter and is folded: `#[comptime.deferred]` stops that
  (`std.testing.assert` has it), and a function without it checks the interpreter, not the target.
- Profiling the compiler: `perf record -F 250 --call-graph dwarf,16384` on a host (the remotes
  have `perf`), then `perf script -F ip,sym` and count the frame above the allocator or
  `clone` to find who copies. A flat profile of `Value::clone` or a hash probe is a copy of
  a whole array per read, or a table scanned per query, not the cost of the work itself.
- A component crate's `export` feature defines the library's entry symbol `vun_component`, in
  the rlib too: build a component with `--features <crate>/export` on its own package, never with
  `--all-features` or on the cli, which would link several of them into one executable (a
  duplicate symbol at link time).
- A library times what it does through the host's table (`--timings`, `--timeline`): its stages
  and queries appear there on every thread, nested under the `component <kind>` call on the
  thread of the call. A component linked in shares the table and needs no relay.
