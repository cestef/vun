# Handoff

State of Vun as of 2026-09-26 (end of the fourteenth session), for the next agent. Read this, then
`CLAUDE.md` (rules and the agent procedure; on disk but ignored by the user's global gitignore, so
`jj st` never shows it), then `docs/spec/00-principles.md`, `docs/spec/decisions.md` and the spec
files of the area you touch.

## 1. What this is

Vun (`.vun`, CLI `vun`) is a systems language designed by the user, compiled by a Rust
workspace. **No magic**: every primitive is a `= _` declaration of the source package `lang`,
syntax desugars only into `lang`, `std` has no privilege, nothing runs that the program did not
write. Spec in `docs/spec/` (draft 2), Typst whitepaper in `whitepaper/`, Vun sources in `lib/`,
test programs in `corpus/`.

Built so far: lexer, postorder CST with a verifier, error-recovering parser, literal reader, HIR
with resolution, package system (manifests, BLAKE3 Merkle hashes, `.tar.zst` releases, user store,
MVS, `lock.vun`, `.replace`, indexes, `vun pkg add|fetch|update|pack`, `vun cache list|gc`), a
query engine, sema (types as values, compile-time evaluation through MIR and an interpreter, impls,
reflection, code values, coroutines, closures, layout, kinds), MIR with the place-state pass, three
diagnostic renderers, explanation pages for every diagnostic (`vun explain`).

## 2. How the user works (non-negotiable)

- Fast iteration; requirements arrive mid-task and are integrated at once; keep going to the next
  step without stopping; commit each finished batch with `jj` (conventional one-line subjects, no
  AI mention). They choose language surface themselves: ask with concrete options and a
  recommendation (`AskUserQuestion`) when a rule is genuinely theirs, otherwise pick and report.
- Design: one rule per concept, no edge cases, minimal keywords, deep namespacing, lints for
  conventions, full traceability, typed codegen.
- Code: generic and data-driven (no hard-coded names: find `lang` items through tables such as
  `forms`, `borrows`, `iteration`, `early_return`), single source of truth, no magic values or
  flags (enums), structured errors, methods on owning types, a verifier per representation, files
  at most 300 lines with tests in `tests.rs`, clippy pedantic clean, `cargo nextest run`.
- Writing: no em dashes, no unicode arrows in prose. Replies are caveman-terse (a hook); code, docs
  and commits are written normally.

## 3. Repository map

```
lib/lang/lang/syntax/grammar.vun   token kinds, node kinds and parse tables
lib/lang/lang/syntax/desugar.vun   appendix B as data: operators, borrows (with place transitions),
                                  forms, type_prefixes, compound, iteration, early_return
lib/lang/lang/errors.vun           every compiler diagnostic id, with explanation pages in errors/
lib/lang, lib/std                 `lang` (primitives, ops impls, iterators) and `std`
corpus/accept, corpus/reject      programs; reject files carry `//~ id` annotations
corpus/planned                    programs waiting for library APIs: parse, report only unresolved
                                  names (listed as what each waits for); move to accept once clean
corpus/support                    stand-ins for what programs take from drivers and missing std
compiler/<stage>/<crate>          base < front < mid < low < back, link, render < tools
fuzz/                             cargo-fuzz targets `lex`, `parse` (outside the workspace)
whitepaper/                       Typst paper: `main.typ`, `template.typ`, `sections/NN-*.typ`,
                                  `figures.typ` (cetz diagrams, lilaq plots), `refs.yml`,
                                  `vun.sublime-syntax` + `vun.tmTheme` (code), `term.sublime-syntax`
                                  + `term.tmTheme` (```term blocks: `$` commands, diagnostics as
                                  annotate-snippets prints them, IR dumps)
```

The whitepaper states the implementation's numbers (corpus, tests, lines per stage, diagnostic
ids) and quotes real `vun` output in ```term blocks; refresh them when they change.

Generated directories (never edit; edit the `.vun` source and run `cargo gen`, the generator
`vun-generate`, built apart from the compiler): `compiler/front/grammar/src/generated/` (kinds and
parse tables of `grammar.vun`, node slots in `node_field`, `node_slot`, `node_kind_slots`),
`compiler/front/desugar/src/generated/` (the tables of `desugar.vun`, their `item` and `interface`
columns as handles), `compiler/base/diag/src/errors/` (a module per group, `all`),
`compiler/front/sema/src/primitives/` (a module per `lang` namespace, `primitive`, each with its
`item()`), `compiler/base/items/src/{lang,std}/` (a handle per public declaration and namespace:
Rust names a `lang` or `std` declaration only through `vun_items`). Generators emit `generate::Module`s (`Access::{Glob, Path, Impl}`);
`Generator::changes` compares whole directories and removes stale files; generated files obey the
300-line limit.

## 4. Commands

```sh
cargo fmt && cargo clippy --all-targets && cargo nextest run && cargo gen --check && cargo xtask lines
cargo xtask corpus            # verdicts, then coverage and what stops MIR most
cargo xtask size              # builds `vun` as distributed; fails past its budget
cargo run -q -p vun-cli -- check|debug tokens|cst|hir|mir|layout|program <file>
cargo run -q -p vun-cli -- util completion nu      # completions: bash, zsh, fish, nu
VUN_BLESS=1 cargo nextest run -p vun-syntax    # snapshots, only after reading the diff
cd fuzz && cargo +nightly fuzz run parse -- -max_total_time=120
typst compile whitepaper/main.typ whitepaper/vun.pdf
```

## 5. The compiler as built, by area

Front end and infrastructure (stable; see the code for detail): `vun-index` (`IndexVec`,
`into_vec`, `From<Vec>`), `vun-intern` (process-wide `Symbol`), `vun-origin` (stable
`OriginId`s anchored to declarations), `vun-span` (`extension!`, `source_file!`), `vun-diag`
(`Msg` parts, `Diagnostic`, `DiagCtxt`, `Diagnostic::moved`), `vun-query` (decision 176: slot
tables, `Fingerprint`s, lock-free reads, threads sharing a `Db` and waiting on each other's
computations; `query!` declares a query in one block; persistence built but not wired), `vun-syntax` (the literal printer writes `Decl::source` and
`Decl::listed`, one item per line), `vun-hir` (+ `db`: the front end as queries), `vun-command`
(declarative CLI), `vun-build` (manifests, `lock.vun` via `Lock::read`), `vun-pkg` (archive
formats, `Format::PUBLISHED` is `.tar.zst`), `vun-http`, `vun-hash`, `vun-fs` (`Tree`, `Edit`,
`Scratch`), `vun-par` (`Parallel::par_map`: the caller and a thread per core claim guided
runs of items, nested calls run inline; the corpus, the repository sweeps, baseline compilation
and C compiles use it), `vun-harness`, `vun-trace`, three renderers.

**Sema** (`compiler/front/sema`):
- Types (`ty/`): `TyKind::{Primitive, Nominal, Tuple, Record, Closure, Frame, Param}`; `Nominal`
  kinds `Struct`, `Enum`, `Empty` (`TyKind::is_empty`). `ty/relate.rs`: `accepts` (integer
  ranges, bounds, borrows of arrays as slices, borrows of one transition and access for each other
  but where `&static` is written, signatures with markers, records, `Code(T)` covariance, empty
  types anywhere) and `join` (borrows meet in the shorter extent). `ty/borrow.rs`: the table
  `borrows` (`borrowing`, `alike`, `extent`, `TyKind::string` for `&static [N]u8`,
  `Primitive::{at, borrow_mode}`). `ty/marker.rs`: function-type markers (table `fn_markers`).
  `ty/display.rs`: large bounds by powers of two (`Int(0..2^64)`).
- Values (`ty/value.rs`): ints, ranges, types, tuples, arrays, variants, structs, primitives,
  namespaces, `Fn`, `Closure`, `Constructor`, `Linked` (`= _` outside `lang`, needs a `link`
  attribute), `Frame` (`eval::Suspended`), `Code`, `Interface`, `Origin`, `Name`. Parts through
  `ops::Part` (`Value::{part, with_part}`; an item part at a range is a run, `Part::run`).
- Evaluation (`env/`): `Env::eval` for constant forms; calls through the metered `Call` query;
  compile-time calls evaluate arguments as their parameters' types and look methods up through
  borrows; `.name(args)` where a type is expected calls a variant or a constructor function
  (`Env::constructed`); `env/named.rs` gives names their values, a member of a `struct` literal
  inside a compile-time call being that call's (`Env::made_member`), and tuple positions
  (`Env::position`). A member a namespace does not declare is `names.unresolved`
  (`Typer::absent`, unless `Declared::complete` says the namespace computes names). Frames record the interface call they are written as (`Frame::written`), so
  `$F: Async(A)` infers `A` (`Env::implemented`).
- Typing (`typeck/`): the `Typer` per `Instantiation` (`Typed` query): expected types, patterns,
  loops, `$for`/`$if` steps (`staged.rs`; `Typer::undecided` counts `$if`/`$for` not decided,
  under which `$e` is typed but not run), `$e` run while typing where all it reads is known,
  traps and library reports from `$if`, `$for`, `$e` and instantiated signatures reported at the
  root (`Typer::reported`, evaluating with the types found so far, `Typer::evaluating`), library
  reports moved to the call that led to them (`Typer::absorb`), quotes and splices, `?`,
  closures, coroutines, omitted `$` arguments (`omitted.rs`), methods and collisions
  (`methods.rs`; `required_member` for constrained parameters and frames, a frame answering its
  interface's `resume` and `end` and having its impls' methods), records and tuple positions
  (`record.rs`), `unsafe` (`safety.rs`), string literals `&static [N]u8`.
- Layout (`layout/`): the query `Shapes` gives each type's `Shape` (`Layout` with its `Niche`,
  and `Parts`: sequence offsets, array stride, bit offsets, or an enum's `Tag` (`Alone`, `Index`,
  `Niche`) and payload offsets); `Shapes::layout` for size and alignment; `Shape::text` is the
  `vun debug layout` format.
- Items: `Declared`, `DeclValue`, `DeclType`, `TypedDecl`, `Members`, `MemberType`,
  `MemberValue`, impls and witnesses (`witness/`: targets are types, type constructors or
  interface constructors; a type's witnesses also come from the package declaring it,
  `Env::impl_files(home)`), visibility, reflection (`lang.meta.{fields, items, name_of, size,
  align, source, span}`), layout, kinds (`Kinds`: `&static` is free), `check/`.
- Traps (`trap.rs`): `Reason` includes `Reported { message, at }` (`lang.diag.error`), shown as
  `comptime.reported` with the library's message.

**Narrowing** (decision 156, `typeck/narrow.rs`): `Facts` of a condition (the table
`comparisons`, `Compared` on `Comparison` in `int/compare.rs`), `Typer::narrowed` overlaid on
locals by `name()`, scoped with `narrowing`, extended after an `if` that cannot fall through
(`after`). `lang.ops` iterators count through narrowed immutable bindings.

**Typed tables** (decision 149): borrow modes are `vun_desugar::Borrow` (a row of `BORROWS`:
`written`, `of`, `plain`, `lasting`, `item`, `before`, `after`, `access`, `extent`, `alike`);
HIR `ExprKind::Borrow { mode: Option<Borrow> }` (`None` is reported as `syntax.unknown_borrow` by
resolution); generated tables of several columns are row structs read by field
(`ITERATION.first()?.resume`); `lang` members in tables are handles (`vun_items::lang::Step::yielded`,
`lang::ops_Try::branch`), with `Item::parent` and `Item::symbol`. The literal reader reads a type
constructor as `Literal::Made`, so handles exist for its members.

**Operators** (decision 144): no operator rule in the compiler. `typeck/operator.rs` finds the
function of the operator's interface for the left operand's type (`Env::implemented_by`) and
types it as a call through `typeck/apply.rs` (shared with written calls: `Callee`, `Passing`,
`Argument`), recorded in `TypedBody.operators` by `Operation` (expression, compound place, pattern
test); `env/operator.rs` evaluates by the same lookup; MIR `build/operator.rs` lowers to a call.
Integer and float witnesses name primitives (`lib/lang/lang/{int,float}/ops.vun`); their
values and ranges are `apply/int.rs`, bit ranges `int/bits.rs`. `Callable` (a `Func` or a
primitive) gives any callee's `$` parameters and borrow modes (`Written`).

**MIR** (`compiler/mid/mir`): places with projections (`Deref`, `Field`, `Index` (a local holding
an integer, or bounds for a run of items), `Position`, `Payload`); rvalues `Use`, `Binary`,
`Unary`, `Range`, `Narrow`, `Aggregate`, `Call`, `Ref`, `IsVariant`, `Resume`, `End`, `Repeat`,
`Comptime`, `Quote`; terminators `Return`, `Goto`, `Branch`, `Yield` (with a `cancel` path running
what is pending, ending in `Ended`), `Defer`, `Unreachable`;
`LocalDecl { name, ty, role }` with `Role::{Held, Unpacked, Static}` (a top-level binding's
storage). `build/`: one module per form; receivers take their mode from `MemberType`; a frame's
`resume()`/`end()` lower to `Resume`/`End` (table `iteration`, column `end`); a call of an empty
type ends its block; `xs[a..b]` through a borrow is a borrow of the run. Unlowered forms are
`Unbuilt { at }`; `Body::verify` runs on every lowered body in the tests. Temporaries are typed
from what they hold (`build/temp.rs`); place types along projections are `Place::types`/`ty`.
`for p in e` and `yield ..e` end their frame on every exit (`Pending::End`). `Lowered` (on
`Instantiation`): `env`, `called` (a call's instantiation from its constant `$` arguments);
`Instantiation::new` makes one instantiation per function with no `$` parameter bound.
`Concrete` (query): a body for code generation, compile-time values as constants propagated into
operands, every local typed, else `Unready` (`Unbuilt`, `Unevaluated`, `Untyped`). `Programs`
(query, `program.rs`): the exports of a root file (`Func::export`, members of the struct types
its bindings evaluate to) and every instantiation and link symbol they reach; `vun debug program`.
`hello.vun` reaches 27 functions and `ffi/export.vun` 4, all concrete (a driver test holds every
corpus program to it). Operators record their result type (`Operator::result`); a coroutine's
result local has its `done` type; `$for _ in ...` unrolls without a binder.

**Passes** (`compiler/mid/passes`): `places/` (uninitialized reads, joins, `&out`, moves of linear
values, `End` consumes, `place.linear_leak` at returns, `place.move_out_of_borrow`, `Defer`).
`borrows/`: loans from `Rvalue::Ref` flow into locals (a call's scoped result holds its
arguments' loans but those passed to `&temp` parameters, per `Func::extents`); a `&temp`
parameter is a `Source::Lent` loan that conflicts with nothing and escapes when it reaches the
result; conflicts by the column `access`; loans of own storage (not `Role::Static`) in the result
escape; `free.rs` (`borrow.in_free_value`). `panics.rs` (`#[panic.never]`), `types.rs` (place
types along projections).

**LIR** (`compiler/low/lir`, decision 150): `Unit` (functions, data with relocations, imports,
`sites`, `pointer_bits`), `Function` (signature with `Conv` and `AbiType::{Scalar, Record,
Memory}`, slots, blocks with parameters, instructions, values), `Builder`, `Unit::verify`
(dominance, types, targets, origins), the text format (`Unit::dump`).

**Lowering** (`compiler/low/lower`, decision 151): query `Lowered` (root file to
`Result<Unit, Unlowered>`), `vun debug lir`. `Types::repr` (zero, scalar, memory), signatures,
`func/` per concern: places (`Reached` with a slice's length), constants (`Lowering::encode`),
conversions (ranges, arrays item by item, enums variant by variant), rvalues, calls (functions,
imports, `lang` primitives with link symbols), primitives (`lang.int` arithmetic and comparisons,
narrowings, `lang.mem`, `lang.bounds`), bounds and runs, traps, coroutine frames (`Frames`,
`frame/`: `resume` and `end` from one body). `Bounds(T)` at run time is its low bound, a second
bound and a `BoundsEnd` byte (`Open`, `Before`, `Through`). `Framing` gives sema frame layouts
(`vun_sema::layout::Framers`, installed by the driver next to the interpreter).

**LIR analyses** (`vun_lir::analysis`, spec 13 section 8.1, after TPDE): `Cfg`, `Loops` (Wei et
al., irreducible loops included), `Order` (reverse postorder, loops contiguous, `span`, `next`),
`Liveness` (per value: first and last position, `through`, uses), `Moves` (parallel copies with a
typed temporary per cycle). Operation facts (`facts.rs`): `reads()` and `domain()` of `BinOp`,
`Cond`, `CastOp`, `Cond::unsigned`. Signatures pass integers as `AbiType::Int(Scalar, Ext)` (the
verifier rejects an integer as a bare `Scalar`); a terminator never jumps to one block twice with
different arguments (`Reason::Edge`). A link symbol the program exports is called directly
(`Lowering::linked`). Comparisons lower in the narrowest storage holding both ranges, narrowings
test only the bounds that cut the source's range: 64-bit code never widens to `i128`.

**Targets** (decision 154): `targets.<system>.<cpu>` (`linux.x86_64`, `bare.cortex_m4f`,
`wasi.wasm32`, `macos.arm64`); `lang.target.Spec` has `arch`, `cpu`, `os`, `abi`, and `c`, the C
model with its `compiler` (`CCompiler`: command, flags, `Atomics`). `vun_target::TargetSpec` is
read from the value (`vun_sema::target::Read`, through handles); `Chosen` (query) picks it by
`Choice::Path` (`--target`) or `Choice::Host` (the row of `targets.hosts` for Rust's
`std::env::consts`); the driver sets the input `TargetOf`, which has no default: widths and byte
order are unknown until it is set (`Unevaluated::Untargeted`, `Why::Untargeted`). Handles exist
for the root file of `targets` only (`vun_items::targets::hosts`). Components name what they
handle with `vun_target::Name`, compared to the target's `Symbol`s.

**C backend** (`compiler/back/c`, decision 153): `ast/` (C99 as data: `Ty`, `Expr`, `Stmt`,
`Item`, typed `Ident`s with `Own` and `Builtin`, `Names` spelling them), `ast/print` (the one
printer), `emit/` (`ints.rs` integer types from the C model with `sizeof` checks, `signature.rs`,
`data.rs`, `func/` with `inline.rs`, `arith.rs`, `inst.rs`, `term.rs`), `lack.rs` (`Missing`, as
`backend.lacks`). Driver: `vun debug code <file> [--backend]`, `vun build <file> [--backend] [-o]`,
`vun run` (`codegen.rs`, `program.rs`); the `gnu` linker adapter links through the target's C
compiler. `vun run corpus/accept/programs/hello.vun --backend c` prints `hello` (a driver test).

**Baseline backend** (`compiler/back/baseline`, decision 157, spec 13 section 8.3): `x86_64/encode`
(`Asm`: typed instructions to bytes, labels patched at the end, `CodeReloc`s), `x86_64/abi.rs`
(`Frame::of`: where a signature passes each value, System V records), `x86_64/func/` (the
single-pass compiler: `values.rs` assignments, homes, recomputed values and eviction; `inst.rs`,
`memory.rs`, `call.rs`, `term.rs`), `elf/` (`Object` as data, `write`), `object.rs` (a unit as one
object). Compares fuse into their branch (`FnCompiler::fused`), registers carry into a
single-predecessor block emitted next (`kept`), frames are reported while emitting
(`ArtifactSink::frames`, `Emitted`, `Compiled::stack`); values live across loops hold callee-saved
registers of their own (`func/fixed.rs`, decision 166).

**Attributes** (decision 163): `lang.Attribute(on)` with `lang.Place`; `check/placed.rs` types
each attribute (`Typer::typed`) and reports `attr.misplaced`; `sema/src/attributes.rs`
(`Attributes` of `Func`, `Nominal`, `DeclRef`: `get`, `count`, `text`, `markers`, `abi`,
`stack_max`) is how items' attributes are read.

**Floats** (decision 167): `vun_sema::float::{Binary, Format}` (exact IEEE arithmetic over
`Integer`, one rounding routine for every format, decimal parsing); `Value::Float`; literals typed
by the expected float type (`typeck/infer.rs`); `apply/float.rs`; lowering `func/float.rs`
(`!=` as `==` negated, `total_order` keys) and `func/verdict.rs` (`condition`, `ordering`, which
`Int::compare` uses too); baseline `func/float.rs`, `encode/sse.rs`, ABI carriers (`abi.rs`
`Carrier`, eightbyte classes), `func/carry.rs`, `func/entry.rs`. NaN is canonical (decision 173):
every NaN result is `Binary::nan` (sign clear, quiet bit only); lowering selects it after each
float operation (`FnLower::canonical`); `neg`, `bits`, `from_bits` keep bits. The encoding is
`lang.float.{Bits, bits, from_bits, fraction_bits}`; `rem` and `mul_add` run as Vun functions of
`lang.float.soft` through `#[lang.lower.by(f)]` (decision 174: `Concrete::fold` rewrites the
call), checked bit for bit against sema by `programs/soft_float.vun`. A call of a primitive whose
parameters are all `$` ones is compile-stage and, for an integer, typed by its value alone
(`Callable::folds`, `typeck/apply.rs`); a `$` binder's place is a copy of its value.

**Terminal output and the command line** (decision 181): `vun-term` (base) holds output:
`Text` (pieces naming a `Style`; `Text::of`, `Text::markdown` for explanation pages), the one
palette (`Style::ansi`, cargo's), `Table` (`Column::left|right`), `Verb` status lines
(right-aligned in 12 columns; `Verb::under` for lines beneath), units (`Elapsed`, `Bytes`,
`Noun::count`), `ColorChoice`/`Env`/`Stream`, `Shell { out, err, verbosity }` with `status` and
`page` (the pager, `$PAGER` or `less -FRX`). Nothing writes ANSI by hand. `vun-command`:
`Command::run|group` builders with `.params`, `.sets(&[OptionSet])`, `.aliases`, `.examples`;
`Param::flag|option|positional` with `.short`, `.about`, `.values(Values::{Text, Path, Choice})`,
`.arity(Arity::{One, Optional, Many})`, `.env`; parsing validates choices, fills variables, and
takes words after `--` as positionals; `-h` short help, `--help` long help, paged;
`Program::completion(Dialect)`; `Program::main` over a `Context` (`Session`, or `DiagCtxt` for
xtask and generate). Global options `--color`, `-q`, `-h`, `-V`; the driver's `COMPILE` set
(`--target`, `--diagnostics`, `--timings`) on build, run, check and debug. `Session::emit` shows
and drains diagnostics (`shown_errors` keeps `failed`); `Session::conclude` ends a compiling
command with `Finished`/`Failed`; `Session::prepare` fetches and locks before compiling (no `vun
pkg fetch`); `commands/status.rs` holds the verbs and `HashText` (hashes as jj ids). Logs are
styled per level, under `VUN_COLOR`.

**Developer tooling** (decision 168): commands organized as jj's (`vun debug ...` for every dump
and `compare`, `vun util`, `vun toolchain components`); `Session::run_each` and
`compare::Ran::agree` (the backend tests use them; their programs are files in
`compiler/tools/driver/tests/driver/programs/`, runnable with `vun debug compare`);
`BackendInfo.readiness`; `Why::Primitive` names an unlowered primitive; `vun debug types <file>`
prints each expression's type and stage in every reached instantiation, from any file, marking
untyped expressions and operators without a function; `VUN_LOG` traces `mir`,
`lower` (per statement), `baseline` (per instruction) and `c` per function.

**Legalization** (decision 169): `vun_lir::legalize` (`Unit::legalize(Legalize { widest,
endian })`, `halves.rs` pair arithmetic, `rewrite/` per function (instructions, calls, terminators), `divide.rs` the division
helper); the driver legalizes a copy of the unit per backend in `Session::code` (`Coded.unit`),
from `BackendCaps::int_bits`. `programs/wide.vun` checks it through both backends. `f16` is
promoted there too (decision 175, `BackendCaps::half`): held as `i16` bits, computed in `f32`
between `legalize/float16.rs`'s exact widening and rounding (`rewrite/half.rs`);
`programs/half.vun` checks every operation against sema bit for bit.

**Stack bounds** (decision 162): `Func::stack_max` and `DeclRef::stack_max` (through `Env::count`)
set `Function::stack_max` and `Import::stack_max` in LIR; `vun_lir::analysis::StackUse` walks the
call graph; the driver checks after compiling (`driver/src/stack.rs`). An import's bound is the
largest its declarations in the program state (`Env::linked_stack`, decision 164), so a program
declares its trap handler's. Open: `stack.depth(k)` for bounded recursion. `vun run <file> --backend baseline` runs hello.vun; the driver's backend tests build every
program through `BACKENDS` (C and baseline) and require equal results. `Lack`/`Missing` live in
`vun-backend`.

**Interpreter** (`compiler/mid/interp`): runs MIR on compile-time values; shared borrows read as
values; exclusive borrows are loans (`Value::Loan`) that `.*` follows (`places.rs`), passed to
calls as what they reach and written back from `Metered::written` (`loans.rs`, decision 135);
frames suspend and resume; `End` of a suspended frame runs its cancel path (`Body::cancel_of`).

**Driver and tools**: `Session::check` runs names, types, places, borrows, panics;
`stages::STAGES`; `Coverage`; `cargo gen`, `cargo xtask {corpus, lines, size}` (the budget in `xtask/src/size.rs`,
10 MiB since decision 139 was accepted; the dist build measured 7,660,256).

**Libraries**: `lang` (primitives: `lang.mem` borrows `Ref`..`RefStatic`, pointers, `Cell`,
`cell`, `cell_pointer`, `atomic_*` with `Ordering`, `items_pointer`; `lang.diag.error`;
`lang.meta.items`). `std`: `option`, `result`, `cmp`, `ptr`, `mem` (`Own`, `alloc.Fixed`),
`int`, `seq` (`len`, `enumerate`, `ptr`, `Eq` of slices and arrays), `cmp` (`derive_eq`),
`option`/`result` (`Eq`), `iter` (`Iter`; `enumerate`, `map`, `filter`, `take`,
`zip`, `chain`, `fold`, `count` on every frame), `sync` (`critical` lending `&temp Critical`, `Shared`, `Atomic`,
`Mutex` with a nested linear `Guard`, link symbols `enter`/`leave`), `os` (`System` with `args`,
`Args` over C's `argc`/`argv` (`lang.mem.items_at`) iterable with `for`, `Stream` with a `Writer`
impl, `Exit`, `start` over `os.libc.start`), `mem.alloc` (`Fixed` implements `Allocator`, its own
method `take`), `fmt` (`Writer`, `Format`, `print`
splitting the format at compile time and reporting a mismatched argument count), `diag`
(`Report`, `error`), `task` (`Event`, `Wait` with `also`, `select`, `run`, `block_on`, link
symbol `sleep`), `build` (`Build` records `Action`s, `plan` runs a package's `build`), `lints`
(a namespace per group, with its `level`), `panic`. `lang.target.Spec` is a struct; `lib/targets`
declares one per target.

**Plans** (`sema/src/plan.rs`, driver `plan.rs`): the `Planned` query takes the declaration to
run from the requirements of `std.build.Package` and calls `std.build.plan` with it; checking a
package's `build.vun` runs it (stage `plan`), `vun debug plan` prints its actions as literals
(`Value::literal`). Nothing carries the actions out yet.

**Naming types** (decision 158): `lang.meta.Frame(f)` is a coroutine function's frame type (the
result of its signature; `Reason::NoFrame` otherwise); `lang.meta.TypeOf` is `lang` source over a
`Code(T)` parameter, so it types any expression without running it. A quoted argument infers `$`
parameters as `Code` of its type (`Callable::quotes`); compile-time calls take the `$` arguments
typing inferred (`TypedBody.inferred`, `Env::inferred`, `Env::call_value`). Meta primitives apply in
`apply/meta.rs`.

**Trap sites at compile time** (decision 159, `apply/report.rs`): `lang.panic.fail` traps with
`Reason::Failed(Failure::{Message, Assertion})`, `lang.panic.trap` with `Reason::Trap`; a reason
naming a place (`Reason::place`) is reported there. `std.testing.assert` fails with
`.assertion(cond.source(), cond.span())`, so `$assert` is checked.

**Calls and expansion** (decision 160): `Env::passing` (`env/passing.rs`, `Passed`) is the one
reading of a call's arguments; `Env::call_value` evaluates it, MIR `Builder::passed` lowers it (`$`
arguments as `Rvalue::Comptime { expected }`, `Rvalue::compile_time` evaluating both `Comptime` and
`Quote`). Parameter types and quotes: `Written::{types, quotes}`. The interpreter's environment is
`Instantiation::running` (typed). Calls passing code expand (`build/expand.rs`, `Remap` in
`remap.rs`); `$c` reading caller locals is `Rvalue::Splice`, built by the expanding caller.
`lang.panic.fail` lowers to a trap site with its message (`vun_lir::Site { at, msg }`). `_ = e` is
lowered (`build/assign.rs`).

Progress: corpus 213 passed, 0 pending, 10 planned, 30 shown, 0 failed; 85% of accepted body
expressions typed; MIR builds 87 of 91 run-time function literals, LIR lowers 254 of 254 reached.
282 tests, including one that checks
every `lang` and `std` file cleanly.

## 6. Decisions

Proposed also: 155 (frame layout), 156 (narrowing, runtime bounds, frame layouts in sema), 157
(the baseline backend). Proposed: 159 (trap sites at compile time), 160 (calls read one way, expansion, run-time `fail`),
161 (baseline fusion, fall-through registers, frames reported). Accepted: 162 (stack bounds, imports
declaring theirs), 163 (attribute places, one reading of attributes),
164 (an import's bound from the program's declarations), 165 (`symbols` structs); proposed: 166
(callee-saved registers for loops), 167 (floats), 169 (legalization of wide integers), 174
(`lower.by`, the float encoding, folding primitive calls typed by value), 175 (`f16` promoted in
legalization), 176 (query engine), 178 (origins equal by identity, process-wide handle tables),
179 (`.vun` never package content); accepted, details proposed: 177 (query persistence), 180
(timings); accepted: 173 (canonical
NaN, the user's request); accepted: 168 (commands as jj's, debug tools). Accepted: 139 (the size budget, now 10 MiB), 158 (`meta.Frame`, `TypeOf`
over code; details proposed).
Accepted (the user's choices, details proposed): 152 (backends after TPDE, baseline backend
started after C), 153 (the C backend), 154 (targets named by system and CPU, chosen per session).
Accepted: 1 to 84, 91, 92, 98 to 106, 109, 111, 116, 118 (no bespoke formats: `lock.vun`,
`.tar.zst`), 120 (the rename to Vun), 131 to 133, 136 (plans with `lang.meta.Any`), 142, 144
(every operator through impls, integers included). Proposed, awaiting the user: 85 to 90, 93 to 97, 107, 108,
110, 112 to 115, 117 (`std.build` API), 119 (`&temp`), 121 (`std.sync`, `std.os`), 122 (statics
in MIR, constructor calls at compile time, members of made types), 123 (`&static`, tuple
positions, range slices, witnesses in the type's package), 124 (`std.fmt`), 125 (library
diagnostics), 126 (executors, frames' `resume`/`end`), 127 (`std.iter` adapters), 128 to 130, 134, 135, 137 (static borrows of literals, `mut`
statics at compile time), 138 (targets and lint groups as data, member handles), 139 (the size
budget), 140 (`std` iterators, arguments, `Fixed` as an `Allocator`), 141 (values' types, members
of types without a namespace, requirements through witnesses), 143 (receivers after inferred
`$` arguments, lent parameters lent on), 145 (layout placement), 146 (`yield` as an exit, cancel
paths), 148 (`Programs`, `Concrete`), 149 (typed borrow modes and tables), 150 (LIR), 151 (lowering); accepted: 142 (the `lang.lint` view), 147 (namespace
members reported, `corpus/planned`). New language
rules go to `decisions.md` as proposed unless the user approves them.

## 7. Next steps, in order

0. Performance and persistence (the user's current focus). Done: `vun-par`, per-package
   `Shared` databases, lock-free interners, the HIR index, and persisted queries (decision 177):
   `vun-codec` with `Writer`/`Reader`, `Blob`s read in place, `Pooled` handles (symbols,
   interned values, files, origins) written once per file; `vun-codec-derive`; every query of
   hir, sema, mir, passes and lower registered in a `PERSISTED` table per crate
   (`driver/src/queries.rs` lists them), `Parsed` and the inputs by trace. The CLI turns it on
   (`Persistence::On`); tests never read or write saved queries. `vun check hello.vun`: 66 ms
   cold, about 11 ms warm (release, `--timings`), where loading takes about 6 ms (decode 1.3,
   keys 2, restore 1). Timings: `--timings`/`VUN_TIMINGS=1` (decision 180);
   `VUN_LOG=vun_query=debug` names why each memo is computed (absent, a changed read, loaded
   without its value) and which memos are not saved; `vun_origin=trace` lists every origin.
   Logging is off unless `VUN_LOG` asks; `info` tells a run step by step (loading, saved
   queries, target, checked, coded, linked), `debug` each query computed and why, packages read,
   files lowered, functions built, lowered and compiled, `trace` MIR statements, LIR
   instructions, selected instructions, origins and query keys. Next, in order: (b) the file never shrinks: memos of deleted
   files and stale keys stay until the file is removed; compact it when it grows past twice its
   live size; (c) writes are not atomic (two `vun` runs at once can leave a truncated file,
   which is then skipped); write a temporary file and rename; (d) the rest of the performance
   audit: cheap interfaces for large values (typed bodies, MIR, shapes: store a fingerprint
   when built), `FileRef.path` as a `Symbol`, `Instance` keys hashed once, `Call`/`MeteredCall`
   hashing the value twice, the interpreter copying whole values per projection, `Integer`
   small-int form and `Value::Bytes`, witnesses per (file, type), `Primitive::at` and
   `Borrow::of` linear scans, SipHash maps in the typer, `Attrs` as a query; (e) mimalloc was
   not tried (allocation is about 20% of a cold check); (f) `par_map` spawns scoped threads per
   call (no `unsafe` is allowed for a persistent pool); measure whether that matters.
1. Baseline backend next: float conversions (they must give the canonical NaN, decision 173;
   to `f16` only from `f32`, decision 175), data
   objects aligned past every C type in the C backend (only slots are handled), and
   128-bit integers (pairs, helpers in Vun), DWARF lines, arm64. Backends: memory regions from a program's build options into `LinkPlan` (the `gnu`
   adapter writes scripts from them), and a bare-metal runtime to link against. A slice value (`xs[a..b]` not borrowed)
   reaches lowering as `backend.unlowered` ("no layout") instead of a sema error.
2. `std`: `std.task.Pool`, `std.testing.Sim`; a Linux runtime without the C library; text
   (`Str`) and collections; `print`'s `.fixes`. Generic bodies
   over a constrained `$A: Allocator` are neither typed nor lowered (`a.alloc(...)`), see 5 and 6.
3. Leaks at scope exits inside a body (found at the function's return only today), `fixed` and
   `scoped` kinds, frames' `end` running their defers (MIR `End`, the interpreter).
4. Borrow rules left: top-level bindings as free values, frames' region from their parameters, a label where a conflicting borrow is
   used later.
5. `vun build`: carry out a plan's actions in order (decode them by the `vun_items` handles of
   `std.build.Action`'s variants), starting with `program` once a backend exists; a program's
   options as a typed record (`root`, `target: lang.target.Spec`, `deps`, `memory`).
6. MIR gaps (`cargo xtask corpus` lists them): `==` in two corpus functions whose operands are
   not typed, `.{ }` and `.name` in
   plain instantiations of generic bodies, or-patterns that bind, `lang.simd.chunks`.
7. Sema gaps: a library report from a range primitive (rule 8) carries a "while evaluating"
   frame inside `lang.int`; traps inside a callee's instantiated signature are not reported; impl
   member types
   against requirements; exhaustiveness (decision 29),
   `#[layout.offset]`/`#[layout.c]`, frame layouts, markers checked beyond `unsafe` and
   `panic.never`.
8. Evaluator: its own stack (deep chains of compile-time calls overflow the host stack); loans
   inside aggregate arguments (`select`'s tuple of `&mut`) and frames holding what they were lent.
9. Packaging: signed caches (decision 63), planned builds,
   `vun pkg why`; `.workspace` visibility.

## 8. Pitfalls

- Origins are equal by identity (decision 178): a new way of lowering that makes two entities
  from one piece of syntax must give each its own origin (`Lower::own`), or two nodes become
  one key; `Hir::verify` catches it. Do not order by `OriginId` (it has no order); syntax nodes
  order by place.
- Tables that handles index (`SourceFiles::global()`, `OriginTable::global()`) are the process's.
  Anything interned that holds a handle is shared by every session in the process.
- Persisted queries need unique `Query::NAME`s (registering two panics) and codecs on their key
  (and value, with `Db::persist`). A query whose values are set is `INPUT`. Saved queries live in
  `.vun/queries/` beside the root file: delete it to start cold.

- The shell may inherit `CARGO_TARGET_DIR` from an old environment: then `cargo build` writes
  elsewhere and `./target/debug/vun` is stale. `unset CARGO_TARGET_DIR` first.
- Nothing has a target until the driver chooses one (`Session::target_for`, on the first
  `file_ref`); code outside the driver that queries layouts sets `TargetOf` itself.
- Run light checks while working (`-p <crate>`, one test); the full check line once per batch.

- The project was renamed from Overt to Vun (crates `vun-*`, CLI `vun`, sources `.vun`, `VUN_*`
  variables); the extension comes from `vun_span::extension!`, file names from `source_file!`.
- `or` is a keyword: a field cannot be named `or` (the `Wait` chain is `also`).
- `$if` cannot bind a pattern (`$if .some(i) = x` does not resolve `i`); carry the value instead.
- A test module named `std` would shadow the Rust crate: the driver's is `stdlib.rs`.
- Children namespaces are not in scope by bare name inside their parent: write `std.os.libc`.
- Private (`_`) fields are visible only inside their own struct literal: a constructor or an
  accessor goes in that struct (`Critical.run`, `Stream.of`, `Mutex.Guard.acquire`).
- Crates bake in `env!("CARGO_MANIFEST_DIR")` (`vun_harness::repo_root`), and cargo does not
  rebuild them when the repository moves: run `cargo clean` after a move. The target directory is
  the repository's own `target/`.
- `cargo gen` never depends on what it generates for others (decision 132): change a table or a
  declaration, run it, then fix the consumers the compiler points at. The session that runs it
  must use the repository's `target/`: a `CARGO_TARGET_DIR` inherited from an older environment
  builds elsewhere.
- Generated code is built with `vun_syntax::make` (`File`, `Item`, `Expr`) and printed by one
  writer; never format Vun source as text. Declarations of `lang` and `std` are named by their
  `vun_items` handles, never by strings.
- `cargo fmt` rewraps code: an edit script matching old text can silently miss afterwards; check
  that replacements applied (assert in the script).
- Python heredocs with Rust escapes are fragile; prefer the Edit tool for backslashes. A script
  that opens a file for writing before reading truncates it.
- Query values must hash by content (`BTreeMap`, not `HashMap`); reports are data, never emitted
  by a query. A query whose cycle is possible needs a `cycle` rule (the default panics).
- Checks report only what they know: unknown types and `Unevaluated` values are silence. A stage
  lists exactly the ids it implements, or corpus files fail instead of pending.
- `VUN_BLESS` blesses whenever set, whatever its value.
- Attributes are written bare only from the namespaces the std prelude lists (`kind`, `layout`,
  `abi`, ...); a new compiler attribute goes in one of those, never in a namespace like
  `lang.mem` that users reach as `std.mem`.
- Kind attributes go on the type's literal, also in a constructor's tail:
  `Own := ($T: type) -> type { #[kind.linear] struct { ... } }`.
- Inside a `lang` namespace file, its own names shadow `lang`'s: `never` in `lang/panic.vun` is
  the attribute `panic.never`, so write `lang.never` there.
- Names in `lang` resolve within `lang` (its prelude is `lang`, `lang.int`); `true`/`false` are
  `lang` names (`bool.true`, `bool.false`) in the std prelude.
- Integer arithmetic is exact: `x * 2` for `x: u32` does not fit `u32`; corpus programs are fixed
  to the rule, not the checker to the program.
- Driver integration tests are one binary (`tests/driver/main.rs`, a module per area); tests use
  scratch stores (`VUN_STORE`); manifests in tests are written with `Manifest::source`.
- A second `jj` workspace needs its own `CARGO_TARGET_DIR`; run `jj st` in it before rebasing.
- Inside `lang`'s operator impls, and anything they call, no operator may be written: it would
  call the same impl again. Primitives and `match` on literal patterns only.
- A witness's function may be named by a primitive (`add := lang.int.add`); a member of a
  witness named like a module-level function shadows it inside the struct (write the full path,
  `lang.meta.concat`).
- Sema unit tests (`compiler/front/sema/src/tests.rs`) run on a stand-in `lang` without operator
  impls: anything using an operator is tested in the driver (`tests/driver/typing.rs`).
