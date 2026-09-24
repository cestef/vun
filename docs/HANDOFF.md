# Handoff

State of Overt as of 2026-09-24 (end of the sixth session), for the next agent. Read this, then
`CLAUDE.md` (rules and the agent procedure; on disk but ignored by the user's global gitignore, so
`jj st` never shows it), then `docs/spec/00-principles.md`, `docs/spec/decisions.md` and the spec
files of the area you touch.

## 1. What this is

Overt (`.ov`, CLI `overt`) is a systems language designed by the user, compiled by a Rust
workspace. **No magic**: every primitive is a `= _` declaration of the source package `lang`,
syntax desugars only into `lang`, `std` has no privilege, nothing runs that the program did not
write. Spec in `docs/spec/` (draft 2), Typst whitepaper in `whitepaper/`, Overt sources in `lib/`,
test programs in `corpus/`.

Built so far: lexer, postorder CST with a verifier, error-recovering parser, literal reader, HIR
with resolution, package system (manifests, BLAKE3 Merkle hashes, `.ovpkg`, user store, MVS,
`overt.lock`, `.replace`, indexes, `overt pkg add|fetch|update|pack`, `overt cache list|gc`), a
query engine, sema (types as values, compile-time evaluation through MIR and an interpreter, impls,
reflection, code values, coroutines, closures, layout, kinds), MIR with the place-state pass, three
diagnostic renderers, explanation pages for every diagnostic (`overt explain`).

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
lib/lang/lang/syntax/grammar.ov   token kinds, node kinds and parse tables
lib/lang/lang/syntax/desugar.ov   appendix B as data: operators, borrows (with place transitions),
                                  forms, type_prefixes, compound, iteration, early_return
lib/lang/lang/errors.ov           every compiler diagnostic id, with explanation pages in errors/
lib/lang, lib/std                 `lang` (primitives, ops impls, iterators) and `std`
corpus/accept, corpus/reject      programs; reject files carry `//~ id` annotations
corpus/support                    stand-ins for what programs take from drivers and missing std
compiler/<stage>/<crate>          base < front < mid < low < back, link, render < tools
fuzz/                             cargo-fuzz targets `lex`, `parse` (outside the workspace)
```

Generated directories (never edit; edit the `.ov` source and run `cargo xtask generate`):
`compiler/front/grammar/src/generated/` (a module per enum and table, node slots in
`node_field`, `node_slot`, `node_kind_slots`), `compiler/base/diag/src/errors/` (a module per
group, `all`), `compiler/front/sema/src/primitives/` (a module per `lang` namespace,
`primitive`). Generators emit `generate::Module`s (`Access::{Glob, Path, Impl}`);
`Generator::changes` compares whole directories and removes stale files; generated files obey the
300-line limit.

## 4. Commands

```sh
cargo fmt && cargo clippy --all-targets && cargo nextest run && cargo xtask generate --check && cargo xtask lines
cargo xtask corpus            # verdicts, then coverage and what stops MIR most
cargo run -q -p overt-cli -- check|dump tokens|cst|hir|mir <file>
OVERT_BLESS=1 cargo nextest run -p overt-syntax    # snapshots, only after reading the diff
cd fuzz && cargo +nightly fuzz run parse -- -max_total_time=120
typst compile whitepaper/main.typ whitepaper/overt.pdf
```

## 5. The compiler as built, by area

Front end and infrastructure (stable; see the code for detail): `overt-index` (`IndexVec`,
`into_vec`, `From<Vec>`), `overt-intern` (process-wide `Symbol`), `overt-origin` (stable
`OriginId`s anchored to declarations), `overt-span`, `overt-diag` (`Msg` parts, `Diagnostic`,
`DiagCtxt`), `overt-query` (verifying traces; persistence built but not wired), `overt-syntax`,
`overt-hir` (+ `db`: the front end as queries), `overt-command` (declarative CLI), `overt-build`,
`overt-pkg`, `overt-http`, `overt-hash`, `overt-fs` (`Tree`, `Edit`, `Scratch`), `overt-harness`,
`overt-trace`, three renderers.

**Sema** (`compiler/front/sema`):
- Types (`ty/`): `TyKind::{Primitive, Nominal, Tuple, Closure, Frame}`; `Nominal` kinds `Struct`,
  `Enum`, `Empty`; `ty/relate.rs` has `accepts` (integer ranges, bounds, borrows of arrays as
  slices, `Code(T)` covariance, constructors applied to `never`) and `join`.
- Values (`ty/value.rs`): ints, ranges, types, tuples, arrays, variants, structs, primitives,
  namespaces, `Fn`, `Closure`, `Linked` (linker-provided `= _` outside `lang`), `Frame`
  (`eval::Suspended`), `Code`, `Interface`. Parts through `ops::Part` (`Value::{part,
  with_part}`).
- Evaluation (`env/`): `Env::eval` for constant forms, calls through the metered `Call` query
  (`eval::MeteredCall`: steps include callees; `fuel::FUEL` or `#[comptime.fuel(n)]` on the
  called function); `env/{attr,code,frame,path,primitive,condition,aggregate}.rs`. Attributes
  are read as data (`Env::{attrs_of, attribute}`).
- Typing (`typeck/`): the `Typer` visitor per `Instantiation` (`Typed` query), expected types,
  patterns, loops, `for p in e` (records `Iteration`), `$for`/`$if` steps (`TypedBody::steps`),
  quotes and splices, `?` (`early_return.rs`), closures and function bodies (`closure.rs`),
  coroutines (`suspend.rs`), primitives typed per call (`Env::primitive_signature`).
- Items: `Declared`, `DeclValue`, `DeclType`, `TypedDecl`, `Members` (expands `$for`/`$if`,
  keeps `_` fields), `MemberType`, `MemberValue`, impls and witnesses (`witness/`: targets may be
  types, type constructors, or interface constructors taking the type first), visibility,
  reflection (`reflect.rs`: `lang.meta.{fields, name_of, size, source}`), layout (`layout.rs`,
  query `Layouts`), kinds (`kinds.rs`, query `Kinds`: `Kind { duplication, region, misplaced }`
  from parts and `#[kind.linear|free]` on a literal or its constructor), `#[kind.owns]` (the
  field `.*` reads through, `MemberTable::deref`), `check/` (the file checker: values,
  impls, visibility, `layout.bits_width`).

**MIR** (`compiler/mid/mir`): places with projections (`Deref`, `Field`, `Index`, `Position`,
`Payload`); rvalues `Use`, `Binary`, `Unary`, `Range`, `Narrow`, `Aggregate` (struct, variant,
tuple, array, closure), `Call`, `Ref`, `IsVariant`, `Resume`, `Repeat`, `Comptime`, `Quote`;
terminators `Return`, `Goto`, `Branch`, `Yield`, `Defer`, `Unreachable`; `LocalDecl { name, ty, role }`.
`build/`: one module per form (blocks, calls with receivers per signature, places, patterns,
control, loops with `for p in e`, suspension, closures, staged, `?`, aggregates, `defer`: the
statement lowered again at each exit behind `Defer { consumes, run, skip }`). Binder locals take
their types from `TypedBody::binders`. Unlowered forms
are `Unbuilt { at }`; `Body::verify` runs on every lowered body in the tests.

**Passes** (`compiler/mid/passes`): `places/` (query `PlaceStates`): uninitialized reads, joins,
`&out` targets, moves of linear values, `place.linear_leak` at returns, `Defer` decided statically
(runs where every consumed local is `init`). `borrows/` (query `BorrowChecks`): loans from
`Rvalue::Ref` flow forward into locals (scoped results hold their operands' loans), locals'
liveness backward; an access overlapping a live loan conflicts when either is exclusive (column
`access` of `borrows`), a write skips loans past a `.*` inside it; loans of own storage in the
result escape; `free.rs` reports values built misplaced (`borrow.in_free_value`). Stage `places` in the
driver; stages declare `Input::{Source, Mir}` so the corpus keeps MIR reports pending in bodies
MIR does not lower (`Session::unlowered`).

**Interpreter** (`compiler/mid/interp`): runs MIR on compile-time values; places are rebuilt to
write parts; shared borrows read as values; `&mut` and writes through `.*` are refused (no memory
model); frames suspend and resume (`frames.rs`).

**Driver and tools**: `Session::check` runs names, types, places; `stages::STAGES`;
`Coverage` (typed expressions, lowered functions, what stops MIR); `cargo xtask {generate,
corpus, lines}`.

Progress (corpus/accept): 57% of body expressions typed, 68 of 105 functions lowered. Corpus:
173 passed, 21 pending, 20 shown, 0 failed. 188 tests. Calls infer omitted `$` arguments
(`typeck/omitted.rs`, `TypedBody::inferred`); variant constructors are values
(`Value::Constructor`); `std.result` gives `map_err`.

## 6. Decisions

Decisions 1 to 84, 91, 92, 98 and 99 are accepted (65 to 84 on the agent's recommendation when the
user asked to settle them; 91, 92, 98 and 99 with the user's choices). 85 to 90 and 93 to 97 are
proposed:
$for in bodies (85), code values (86), quote levels (87), MIR places (88), MIR patterns and typed
annotations (89), linked values and repeats (90), layout (93), place states (94), kinds and moves
(95), `defer` (96), borrow rules (97). New language rules go to `decisions.md` as proposed unless the user approves them.

## 7. Next steps, in order

1. After `defer`: leaks at scope exits inside a body (found at the function's return only
   today), `fixed` and `scoped` kinds, frames' `end` running their defers. `memory/defer.ov`
   waits on `Result.map_err` (step 3).
2. Borrow rules left: `&temp` parameters (results not tied to them), `&static`, top-level
   bindings as free values, frames' region from their parameters, `critical_escape.ov` (needs
   `std.sync`), two-phase borrows if programs need them (decision 97 says no).
3. Library and stand-ins so `types.no_member` on values can turn on (the probe in
   `typeck/infer.rs::member` is off): a real `std.mem.Own` (today a stand-in with
   `#[kind.owns]`), `std.Dyn`, `Result.expect` (needs a panic in `std`), `std.sync` (`Shared`, `critical`), `std.iter`
   adapters (`enumerate`), and stand-ins tied to program types (`Dma`, `Uart`, `System`, `svd`).
4. MIR gaps (`cargo xtask corpus` lists them): run-time splices of code (expansion at the call
   site), `.{ }` and `.name` in plain instantiations of generic bodies, or-patterns that bind,
   `lang.simd.chunks`.
5. Sema gaps: operand checks and `types.no_impl` for operators, impl member types against
   requirements, collision rule, constrained `$W: Writer`, narrowing by comparisons (decision
   35), exhaustiveness (decision 29), `c.span()`, `meta.frozen` (lazy members never cycle today),
   `#[layout.offset]`/`#[layout.c]`, frame layouts, `std.int` `abs` and power-of-two `.wrap`.
6. Evaluator: its own stack (an unbounded chain of distinct compile-time calls overflows the host
   stack before the budget), a memory model for `&mut` at compile time.
7. Persist the front-end queries (`.overt/queries/<toolchain hash>`); packaging: signed caches
   (decision 63), planned builds, `overt pkg why`; `.workspace` visibility.
8. `names.private`, `binding.hole_value`; fuzz the parser longer.

## 8. Pitfalls

- `cargo xtask generate` is built from the crates it regenerates: when a change breaks the
  generated code or a caller of a changed table shape, make callers tolerant first (tuple
  patterns with `..`), or hand-patch the generated files to the new shape, build, regenerate. A
  new diagnostic id used by the driver needs a temporary existing id first.
- `cargo fmt` rewraps code: an edit script matching old text can silently miss afterwards; check
  that replacements applied (assert in the script).
- Python heredocs with Rust escapes are fragile; prefer the Edit tool for backslashes. A script
  that opens a file for writing before reading truncates it.
- Query values must hash by content (`BTreeMap`, not `HashMap`); reports are data, never emitted
  by a query. A query whose cycle is possible needs a `cycle` rule (the default panics).
- Checks report only what they know: unknown types and `Unevaluated` values are silence. A stage
  lists exactly the ids it implements, or corpus files fail instead of pending.
- `OVERT_BLESS` blesses whenever set, whatever its value.
- Attributes are written bare only from the namespaces the std prelude lists (`kind`, `layout`,
  `abi`, ...); a new compiler attribute goes in one of those, never in a namespace like
  `lang.mem` that users reach as `std.mem`.
- Attributes do not attach to a block's tail expression; put kind attributes on the type
  constructor (`Own := #[kind.linear] ($T: type) -> type { struct { ... } }`).
- Names in `lang` resolve within `lang` (its prelude is `lang`, `lang.int`); `true`/`false` are
  not prelude names: write `.true` with an expected `bool`.
- Integer arithmetic is exact: `x * 2` for `x: u32` does not fit `u32`; corpus programs are fixed
  to the rule, not the checker to the program.
- Driver integration tests are one binary (`tests/driver/main.rs`, a module per area); tests use
  scratch stores (`OVERT_STORE`); manifests in tests are written with `Manifest::source`.
- A second `jj` workspace needs its own `CARGO_TARGET_DIR`; run `jj st` in it before rebasing.
