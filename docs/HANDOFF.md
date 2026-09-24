# Handoff

State of Overt as of 2026-09-24 (sixth session), written for the next agent. Read this, then
`CLAUDE.md` (the user's rules and the agent working procedure; it exists on disk but is excluded
by the user's global gitignore, so it is not in version control), then
`docs/spec/00-principles.md`.

## 1. What this project is

Overt (working name, extension `.ov`, CLI `overt`) is a systems programming language designed by
the user, with a Rust compiler. Aim: embedded to servers, performant, low-level, ergonomic, with
**no magic**: every primitive is an ordinary declaration in a source package `lang`; syntax only
desugars into `lang`; `std` has no privilege; nothing runs that the program did not write.

Status: design draft 2 (spec in `docs/spec/`), a Typst whitepaper draft, a 47-crate Rust
workspace, a lexer, a postorder CST whose verifier also checks named children (node fields), an
error-recovering parser with machine-applicable fixes, a reader of literal declarations (with
tagged values), generated grammar and diagnostic tables, a declarative CLI, three diagnostic
renderers (`terminal` over annotate-snippets, `ariadne`, `short`) and a `build.ov` manifest
reader, and HIR: lowering through node fields, a verifier (the HIR is a tree with origins), a
visitor, and name resolution within a file with the checks that need no types (`names.duplicate`,
`names.shadowed`, `binding.irrefutable_test`), and package loading: manifests, dependencies,
preludes and spreads (`..ns;`, `..ns.{ a, b };`), so names not declared in a file resolve against
its package and report `names.unresolved`. Packages: manifests read and written as literals,
BLAKE3 Merkle hashes, `.ovpkg` archives, one store per user with lockfile roots, minimal version
selection and `overt.lock`, `.replace`, indexes, and `overt pkg add|fetch|update|pack`,
`overt cache list|gc`. Infrastructure for sema: a query engine with verifying traces, a global
symbol pool, a shared origin table, and stable file and origin identities. Every diagnostic has a
checked explanation page (`overt explain`). `overt check <file>` runs every stage;
`overt dump hir` prints the resolved HIR with origins. Every file in `lib/` and `corpus/accept`
parses, lowers, verifies and resolves cleanly. Type checking has begun: annotated declarations
and function bodies are typed (`types.mismatch`, `int.does_not_fit`, `int.width_cap`), with
compile-time calls run by the interpreter.

History (jj, newest last): scaffold; lexer; parser, CST, tables, CLI and corpus; bitwise compound
assignments; corpus grown to 40 accept and 50 reject; modules split and the 300-line limit;
tagged literals and node fields; terminal renderer, `--format` and parser fixes; manifests;
ariadne renderer; handoff; settled open questions (decisions 28 to 44); HIR; name resolution; integer widths in `lang.int`; package loading, spreads and
`names.unresolved`; `overt-suggest` and `overt-hash`; graph, fs and outline crates; BLAKE3 Merkle
hashes and the nested store; `overt pkg fetch`, `.ovpkg` and error chains; file and directory
duplicates, `overt_fs::Edit` and `Scratch`; `overt pkg add` and `pack`, literals written back
(`Literal::source`, `laid_out`, `Decl::insert`, `Edit::apply`); the executable bit in snapshots,
tree hashes and `.ovpkg` 2; nextest; MVS and `overt.lock`; the shared user store, labels and
`cache gc`; `overt-query`; the global symbol pool; the shared origin table; typed literals and path
calls in the literal reader; stable file and origin identities; `overt explain` and the explanation
pages; `.replace`; `overt pkg update` through indexes. Fifth session: lockfile fixes without
cascades (decision 65), `nesting_too_deep` wording, persisted query memos, exact integers and
intervals in `overt-sema`, primitives generated from `lang` (decision 66), MIR and the
interpreter, body typing with `lang.Fn` (decision 68).
The user asked for commits per batch: commit each finished batch with `jj`.

## 2. How the user works (non-negotiable)

`CLAUDE.md` has the rules and a seven-step working procedure; follow it. In short:

- They iterate fast, add requirements mid-task and expect them integrated immediately, and they
  want the agent to keep going to the next step without stopping. They like research-backed
  decisions (clone references into `~/code/strangers`, fetch pages with `curl`, never WebFetch)
  and dry, behavioral names, never puns.
- Design: no magic, one rule per concept, no edge cases, minimal keywords, deep namespacing,
  conventions as default-on lints, full traceability, typed codegen, pedantic diagnostics.
- Code: generic, reusable and pluggable; **no hard-coded names or special cases**, behavior driven
  by declared data; single source of truth (generate, do not restate); no magic values; typed
  messages; structured errors; verifiers for every representation; fuzzing only in `fuzz/`.
- Tooling: `jj` only; conventional one-line commits without AI mentions; commit each finished
  batch and keep going to the next milestone without stopping. Tests run with `cargo nextest run`.
- Methods on the owning type, never orphan helper functions; follow the surrounding conventions.
- Writing: never em dashes; no unicode arrows in prose. Caveman-style terse replies are enabled
  by a hook; code, docs and commits are written normally.

## 3. Repository map

```
CLAUDE.md                 rules and agent procedure (untracked by global gitignore; keep updated)
README.md                 overview and commands
Cargo.toml                workspace: members compiler/*/*, exclude fuzz; workspace deps; lints
docs/spec/00..14, appendices.md, decisions.md    the specification
docs/research/, docs/archive/                    survey behind draft 2; draft 1
whitepaper/               Typst paper; `typst compile main.typ overt.pdf` works
compiler/<stage>/<crate>  the Rust workspace (section 5)
lib/                      Overt sources: lang/, std/, targets/, boards/
lib/lang/lang/syntax/grammar.ov   THE grammar: token kinds, node kinds, enums, parse tables
lib/lang/lang/syntax/desugar.ov   appendix B as data: operators, borrows, forms
lib/lang/lang/errors.ov           every compiler diagnostic id
corpus/accept             programs that must compile (CST snapshots in compiler/front/syntax/tests)
corpus/reject             must-fail programs with `//~ id` annotations (`cargo xtask corpus`)
fuzz/                     cargo-fuzz crate (outside the workspace): targets `lex`, `parse`
```

Generated files (never edit; edit the `.ov` source and run `cargo xtask generate`):
`compiler/front/grammar/src/generated.rs` (from `lib/lang/lang/syntax/`: `grammar.ov` and
`desugar.ov`, read as one set of declarations) and `compiler/base/diag/src/errors.rs`.

## 4. Language design in brief

Full detail in `docs/spec/`. Decisions 1 to 6 and 24, 26 in `decisions.md` are accepted by the
user; the others are proposals.

- **Seven concepts** (spec 00): bindings, types as compile-time values (`$`), range integers,
  place states, scoped borrows, capabilities as values, suspension.
- **Syntax** (spec 01): 15 keywords; 4 contextual (`in` after a `for` pattern; `out`, `take`,
  `static` after `&`). Bindings `x := e`, `x: T = e`; functions are typed tuple patterns
  `(a: u32) -> u64 { }`; `name: T = _;` is provided from outside; `$` is one stage earlier;
  `#[...]` attributes, `#{ }` quotes; `_` prefix is private.
- **Backslash takes what follows literally** (decision 24, user-approved): `\unsafe` is a name,
  `\"C:\dir"` is a string without escapes, `\\` lines are a raw multi-line string. `"..."` is the
  only escaped string form. `lang.diag.Safety` uses `\unsafe`.
- **Parse rules** (spec 01, section 5, "Parse rules"): patterns are expressions in the CST; in
  `if`/`for`/`match` heads and after `->`, `{` opens the body; typed literals `T{ }` only after a
  name, member or call; `[e]` followed by a type start is `[e]T`; block-like statements end at
  their `}`; comparisons and ranges do not chain; `a..` is open; a group with a typed parameter or
  followed by `->` is a function type, and a function literal with a body.

## 5. Compiler architecture as built

Layering: `base` < `front` < `mid` < `low` < `back`, `link`, `render` < `tools`. Implemented and
tested (the rest are stubs):

| Crate | Contents |
|-------|----------|
| `overt-index` | `Idx`, `index!`, `IndexVec` (push, get, last, iter). Every arena. |
| `overt-intern` | `Symbol::intern`, `Symbol::as_str`: one sharded, append-only pool per process; equal by pointer, hashed by text (BLAKE3), ordered by text. No interner handle is passed around. |
| `overt-origin` | `OriginTable` over `boxcar` (append-only, `&self`: `source`, `declaration`, `inside`, `derive`, `contains`), `Origin` with `Anchor::{File, Declaration, Within}`, `Via`, `Chain`. `OriginId` hashes by its anchored identity (decision 59); lowering anchors origins to declarations (`Lower::declaring`). |
| `overt-span` | `FileId` (hashes by its key: `SourceFiles::add_as`; the driver keys files `<package>@<version>/<path>` through `Session::file_key`), `ByteRange`, `Span`, `SourceFile` (`line_col`), `SourceMap`, `SourceFiles`, `EXTENSION`. |
| `overt-origin` | `OriginId`, `Origin`, `Via`, `OriginTable`. |
| `overt-diag` | `DiagId`, `Severity`, `Msg` (typed parts, `append`), `Label`, `Fix`, `ChildKind` (Display), `Diagnostic` (`label` adds a secondary label), `DiagCtxt`, `DiagCx::error_at`, `DiagSink`, `ErrorGuaranteed`; `Msg::did_you_mean`; `errors` (generated). |
| `overt-suggest` | `closest` (did-you-mean by edit distance), `Lookup` on slices (find by name or the closest name). Generic, no diag types. |
| `overt-codec` | `Encode`, `Decode`, `Malformed`, `take`, `record!` (a struct field by field): LEB128 varints, zigzag for signed, length-prefixed text and sequences, tagged options and booleans. No dependencies. |
| `overt-hash` | BLAKE3 only. `Algorithm` (typed, `name`), `Hash` (`blake3:<base32>`, `parse`, `digest`, `of(bytes)`, `tree(dir, include)`), `Snapshot` (per-file `Entry { hash, mode }` by path, `root` = Merkle tree over path components with directory nodes in a derived key, `changes`, text form, portable plain-component paths), `Change`. Package identity, manifest `.hash`, every cache key. |
| `overt-pkg` | `Mirror` (`parse`, `fetch`), `SCHEMES` (https, http, file), `FORMATS` (`.ovpkg`, `.tar.zst`, `.tar.xz`, `.tar.gz`, `.tar`, `.zip`; `strip` of a forge's top directory), `Archive` (`.ovpkg`: `pack`, `unpack` verifying each file), `Format` rows both read and write (`Format::pack(dir, include, top)`, `Archive::FORMAT` is the `.ovpkg` row), `Contents` (a snapshot's files read from disk under a top directory), `Tar` (`Header` shared by reader and writer: ustar, GNU long names, PAX paths; links refused; paths through `Snapshot::components`), `Zip`, `Zstd` (ruzstd), `Failure`/`Malformed` with sources. `lzma-rs` writes valid but uncompressed xz, so `.tar.xz` packs are larger than `.tar`. |
| `overt-query` | `Query` (`NAME`, `Key`, `Value`, `compute`, `interface`, `cycle`), `Db` (`set`, `get`, `revision`, `persist::<Q>`, `save`, `load`), `Cx` (`get` records reads), `Cycle`, `Persist` (opt-in, keys and values `overt_codec::{Encode, Decode}`), `Unloaded`, `Db::resource::<T>()` (shared append-only tables: `SourceFiles`, `OriginTable`). Verifying traces of interface hashes, early cutoff, one verification per revision; loaded memos are unverified (`verified: None`). Single-flight per thread only. The driver does not save or load yet: that comes with the first persisted sema queries (`.overt/queries/<toolchain hash>`). |
| `overt-sema` | `int`: `Integer` (num-bigint), `Interval` (`add`, `sub`, `mul`, `neg`, `div`/`rem` -> `Divided { range, divisor: Divisor }`, `contains`, `hull`, `bits`, `storage` -> `Storage` or `None` past the width cap), tested exhaustively against small ranges. `primitives` (generated): one module and enum per `lang` namespace (`primitives::mem::Mem`), `Primitive` over them (`ALL`, `namespace`, `name`). `ty`: `Ty = Interned<TyKind>` (`Primitive(p, args)`, `Nominal { literal, args, kind }`, `Tuple`, `Array`; `TyKind::int`, `interval`, `accepts`, `signature` -> `Signature` (`lang.Fn((A, B), R)`), `unit`, Display; a `Nominal` shows the declaration its literal is written in), `Value` (`Int`, `Range(Option<Interval>)`, `Type`, `Tuple`, `Variant`, `Struct`, `Primitive`, `Namespace`; `as_type`). Queries: `item::Declared((Namespace, Symbol)) -> Option<Site>` (through spreads and directories), `decl::DeclValue(DeclRef)` (a `lang` hole is its primitive), `decl::DeclType(DeclRef) -> Known` (annotation or value type, `Stage::Compile` unless `mut`), `typeck::Typed(FnRef) -> TypedBody` (types per expression and reports), `check::Checked(FileRef)` (a `Visitor` over the file: members through `Typer::decl`, bodies through `Typed`). `typeck::Typer` is a HIR `Visitor`: literals, names (locals by annotation or value, others by `DeclType`), namespace members, blocks, `-`, arithmetic by `Interval::binary` with `int.width_cap` on `Stage::Run` intermediates, calls against `Fn` signatures, `$e`; other forms have their parts typed and no type. `Env::{ty, result, signature}` evaluate annotations. `eval::Instance { f: ExprRef, args }` is a compile-time call (the `Call` key); `Env::within(cx, instance)` binds the function's parameters to the arguments, and `Env::nominal` identifies a literal by `ExprRef` plus the instance. `member::Members(Ty) -> MemberTable` (fields with types evaluated in the instance, declarations, `complete` when nothing is generated or spread; `find` -> `Found`). The typer types fields (`p.x`), members of a type's namespace (`Point.origin`), method calls (receiver skipped against the first parameter), and `T{ .x = e }` (fields checked, unknown names `types.no_member`). `ExprRef` and `DeclRef` hold a `Node<Id>` (the index and its origin) and compare and hash by the origin, so keys and types built on them are stable across processes and edits (decision 59); build them with `ExprRef::new(file, hir, expr)`, `DeclRef::new`. `visible`: `Home::{Namespace, File, Struct}`, `Reach::{Public, Home, Within, Package, Workspace}`, `Visible`; `Env::visible(name, attrs, home)` reads `#[visible(...)]` as data (the primitive `lang.visible`, values of `lang.scope` looked up in the namespace declaring `Scope`) and returns what is `Misplaced`; `Env::sees(visible, at)` (`None` for the workspace), `not_visible`, `scope_msg`, `decl_visible`. `MemberTable` entries (`Field`, `MemberDecl`) carry their `Visible`; `TyKind::borrowed` lets `.` look through `&T` (table `BORROWS`). `check/visible.rs` reports `names.visibility` and spread selections of restricted names (moved out of `overt_hir::db::names`, which now only reports unknown selected names). `env::Env` resolves names to sites and evaluates constant forms; anything else is `Unevaluated` and never reported. Stubs: `eval` (`Evaluator` trait), `layout`. |
| `overt-graph` | `Requirements<K>` (`add`, `order`, `OrderError`): the pass manager's ordering. |
| `overt-fs` | `Tree` on `Path`: `files` (recursive, sorted), `entries`. `Modes` on `Path` (`mode`, `set_mode`: `Mode::{Regular, Executable}`); `Relative` (`relative_to`). `Edit` on `Path`: `put` (creates parents), `remove` (absent is removed), `fresh` (empty directory), `replace_with` (move a directory in whole). `Scratch`: a test directory removed on drop. Every filesystem write in the compiler and its tests goes through them. |
| `overt-outline` | `Outline` + `Outlined::node`: indented tree text for the CST and HIR dumps. |
| `overt-grammar` | `generated.rs`: `NodeField`, `NodeSlot`, `Count`, `NodeKind::slots` (children per kind, from tagged entries), `TokenKind` (+ `TRIVIA`, `ATOMS`, `KEYWORDS`, `PUNCTUATION`, `END`, `text()`), `NodeKind` (`LEAVES`, `BRANCHES`), `Context`, `Assoc`, tables `CONTEXTUAL`, `DELIMITERS`, `INFIX`, `PREFIX`, `POSTFIX`, `ASSIGN`, `OPERANDS`, `ELEMENT_TYPES`, `LITERAL_TYPES`. `lib.rs` derives lookups: `punctuation`, `keyword`, `is_trivia`, `infix` (`Prec`, `Infix`), `closer`/`opens`/`closes`, `is_leaf`, `contextual`. |
| `overt-syntax` | `tokens` (SoA tokens + trivia side table, `verify`); `lexer` (`lex`, recovery, `literal(kind, text)` for string values, backslash escapes); `cst` (postorder `(kind, Anchor, size)`, `Anchor::{Token, Missing}`, `children`, `span`, `dump`, `verify` -> `Malformed`); `parser` (`parse`); `literal` (`declarations` -> `Decl { name, doc, value: Literal, span }`, `Literal::Tagged`); `literal::{print, insert}` write values back (`Literal::source`, `Literal::laid_out` within `WIDTH`, `Decl::insert` -> `overt_diag::Edit`); `Cst::fields(n)` -> `Fields` (`get`, `one`, `assigned`), matched against the kind's slots; `children_rev` walks without allocating. |
| `overt-hir` | `tree` (arenas: namespaces, decls, blocks, exprs, pats, binders; `List`/`Entry::Staged` for `$for`/`$if`), `lower` (one `Cst::fields` per node), `visit` (`Visitor` + `walk_*`), `verify` (tree property, origins), `resolve` (scopes: namespace, local, function, signature; `Res::{Binder, Outer { name, ns }}`, `Resolution::parents`; the checks; `Resolution::check_outer` over the `Outside` trait reports `names.unresolved`), `dump`. `Member::Spread` (`..path`, `..path.{ a, b }`; selected names are binders). `Hir::members`, `Hir::binders`, `Hir::path`, `tree::each_entry`. |
| `overt-hir::db` | The front end as queries. Inputs set by the driver: `Package(PackageId) -> PackageData` (name, dir, key, deps as `Result<PackageId, Unloaded>`, prelude, files) and `FileText(FileRef)`. Queries: `Parsed(FileRef)` (tokens, CST, HIR, resolution, reports), `NamespaceAt`, `Members`, `Exports` (cycles give nothing more), `Scope(PackageId)` (names, `poisoned`, reports), `FileNames(FileRef)` (spreads, file/directory duplicates, `check_outer`). Reports are data (`DiagCtxt` in values); an absent dependency carries its prepared diagnostic, returned only where it is needed; the driver absorbs reports, deduplicated. Namespace layout is read from `PackageData::files` (`file_of`, `children`, `holds`, `namespace_of`, `directory`). |
| `overt-command` | Options of a group apply to every command below it; `Program.configure` applies them before a run. Declarative command trees: `Program`, `Command`, `Action::{Run, Group, Planned}`, `Param`/`ParamKind::{Positional, Flag, Option}`, `Args`, `parse` -> `Request`, `Selected`, generated `help`, `run` -> `Outcome`/`Refusal`, `Outcome` -> `ExitCode` (usage = 64). Usage errors are diagnostics with suggestions. |
| `overt-driver` | `stages::STAGES` (implemented stages and the diagnostic groups they report), `Session::check` (runs them: `parsed`, `check_names` over the queries), `commands::PROGRAM` (the whole `overt` tree of spec 12.7, most commands `Planned`; `dump tokens|cst|manifest`, `components` run), `formats::FORMATS` (renderer registry behind `--format`), `Registry`, `Session` (`load`, `read`, `text`, `lex`, `parse`, `lower`, `resolve`, `declarations`, `manifest`, `emit`, `failed`). `packages`: `PackageKey::{Content(Hash), Loose}`, `Package` (deps, prelude), `Session::{package_of, package_at, dep, namespace_at, members, exports, scope, spreads}`, `Scope`/`Outer`, `FileScope` (implements `Outside`), `Session::fetch` -> `Fetched` (graph walk, mirrors in order, staging, verify, commit) behind `overt pkg fetch`, `LIBRARY` (the toolchain library, today the repo's `lib/`). Caches keyed by content hash: manifests and file declarations by text hash, packages by include-tree hash. |
| `overt-cli` | `main` runs `PROGRAM` through `overt-command` and renders diagnostics once at the end. |
| `overt-render-short` | `file:line:col: severity[id]: message`, then `  = help: ...` lines (fixes too). |
| `overt-render-annotate` | default `terminal` format: labels by file, notes, fixes as patches, traces. |
| `overt-render-ariadne` | `--format ariadne`: one color per label role; fixes as help lines. |
| `overt-build` | `index` (`Index::{listing, archive}`, `Listing`), `lock` (`Lock`, `Locked`, `Replaced`/`Replacing`, `FILE`, text form), `Manifest::{replace, index}`, `Replacement`, `Dep::{declare, redeclare}`, `Manifest::{literal, source, identity}`, `Identity`, `EDITION`; `manifest::read` -> `Manifest` (`Version`, `Dep` with span, `Source`, `DepKind`, `prelude_span`), `manifest::declared` (a `build.ov` is a manifest iff it declares `package`), `DEFAULT_DEPS`, `DEFAULT_PRELUDE`; generic record reader reporting missing, unknown (did-you-mean) and ill-typed fields. `store::Store` (`Store::user()` in the XDG cache via `etcetera`, or `Store::at`; `<algorithm>/<2 chars>/<rest>/{package/,snapshot,label}`, `staging/`, `roots/`; `register`, `list`, `collect`, `Label`). `layout`: `MANIFEST`, `SEPARATOR` (from the grammar's `.`), `Location` (namespace on disk: `root`, `member`, `file`, `exists`, `children`, `path_of`). |
| `overt-harness` | `repo_root`, `expectations` (`//~ id [@col] ["fragment"]...` -> `Expected`, or `Malformed`), `Reported`, `compare` -> `Mismatch { missing, unexpected, differing }`, `discover`, `snapshot`, `snapshot_tree` (mirrors a source tree, `OVERT_BLESS`), `Page::parse` (Markdown sections by `##`, `Block { lang, file, text }`) for the explanation checks. |
| `overt-xtask` | `cargo xtask generate [--check]`: pluggable `generate::Generator { source, output, emit }` table (`grammar`, `errors`); shared read/parse/emit/format (quote + prettyplease). `cargo xtask corpus`: runs `corpus/accept` (must be clean) and `corpus/reject` (exact `//~` set) through `Session::check`; a file whose missing ids all belong to stages that do not exist yet is pending (`overt_driver::stages::STAGES`, groups from `errors.ov` namespaces via `DiagId::within`). |
| `overt-mir` | `Body` (locals, params, `ret`, blocks of `Stmt { place, value: Rvalue, origin }`, `Terminator`), `Rvalue::{Use, Binary, Unary, Range, Call, Nominal}` (a `struct`/`enum` literal, typed by the interpreter with its call's arguments, so `Pair(u8)` and `Pair(u16)` differ) (operators as grammar `TokenKind`s), `Operand::{Place, Const(Value)}`, `Place { local }`; `Mir(FnRef)` query lowering a function literal through sema's `Env` (`Unbuilt { at }` for forms not lowered yet: comparisons, `if`, `and`/`or`, assignments); text format (`Display`; `Body::dump(locate)` adds each line's origin, behind `overt dump mir`), `Body::verify` -> `verify::Malformed` (locals, places, jump targets, parameters, origins), checked on every body the repository lowers (`driver/tests/mir.rs`, through `Session::bodies`). |
| `overt-interp` | `Interpreter` implements `sema::eval::Evaluator`: `Frame` runs a body on compile-time values; calls go through sema's `Call` query; operators through `Value::{binary, unary, range}`, primitives through `Primitive::apply`. Installed by `Session::default` in the `Evaluators` resource. |
| `overt-trace` | Own subscriber: `Filter` (`OVERT_LOG` directives, most specific target wins), compact and tree output, span timings. |
| backends, linkers, target, pass, tool | as in the first session (stubs with `const INFO`). |

**CST design.** One node per token (Carbon style), children before parents. A bracketed node is
anchored at its opening token with the closer as a `Punct` leaf; separators and terminators are
`Punct` leaves; `Binding`/`Field` are anchored at `:=`/`:`; absent tokens or operands are nodes
with `Anchor::Missing(next token)`; skipped tokens are `Error` leaves. `Doc` and `Attr` wrap the
thing they annotate. The verifier checks size nesting, leaf arity, a single `File` root, one
owner per token, contiguous subtrees.

**Parser design.** Recursive descent; `List::{Members, Statements, Items(Item)}` drives all
bodies, so `$for`/`$if` work in every list context; node kinds are chosen after children
(postorder). One report per position; missing things are reported at the end of the previous
token; nesting capped at 256; the keyword hint suggests `\keyword`.

Tests: 157, including unit tests per crate, `tests/lexer.rs`, `tests/parser.rs` (all of `lib/` and
`corpus/accept` parse clean and verify, CST and token snapshots mirroring the corpus tree), CLI parsing tests, the
generated-files-current test and the corpus test (no failing corpus file). Fuzz: `parse` ran 600k inputs in 4 minutes
without a failure. clippy pedantic is clean.

## 6. Commands

```sh
cargo fmt && cargo clippy --all-targets && cargo nextest run && cargo xtask generate --check && cargo xtask lines
cargo run -q -p overt-cli -- dump cst corpus/accept/integers/ring_buffer.ov
cargo run -q -p overt-cli -- help dump
cargo xtask generate                       # after editing grammar.ov or errors.ov
cargo xtask corpus                         # corpus verdicts: pass, pending, fail
OVERT_BLESS=1 cargo nextest run -p overt-syntax  # accept snapshot changes, after reading the diff
cd fuzz && cargo +nightly fuzz run parse -- -max_total_time=120   # cargo-fuzz is installed now
typst compile whitepaper/main.typ whitepaper/overt.pdf
```

## 7. Conventions to keep

- New diagnostic: add it to `lib/lang/lang/errors.ov` in its group, run `cargo xtask generate`,
  report with `Msg` parts and `ChildKind::Help`/`Note` children.
- New token, node kind or grammar table: edit `grammar.ov` only, run `cargo xtask generate`
  (twice when the token set changes, since the reader is built from the previous output). Tables
  are typed by their values; a `.name` list resolves to the one enum holding all its names.
- New command: add a `Command` to `overt-driver::commands::PROGRAM` (or to `xtask`), with typed
  `Param`s; never parse arguments by hand.
- New generator: a module in `xtask` with a `const GENERATOR`, listed in `generate::ALL`.
- Files stay at most 300 lines, one concern each (`cargo xtask lines`, also a test). Split a
  growing file into a directory module; unit tests live in a sibling `tests.rs`
  (`foo.rs` + `foo/tests.rs`, or `src/tests.rs` for a crate root).
- Arenas: `index!` + `IndexVec`. Crates: `compiler/<stage>/<name>`, workspace fields, lints,
  workspace dependency entry.

## 8. Next steps

In order:

1. Sema, continued. State: the front end runs as queries (`overt_hir::db`); `overt_sema` has
   types and values, `Declared`, `DeclValue`, `Call` (memoized compile-time calls through the
   installed `Evaluator`), `Checked`; MIR v0 (`overt_mir::Mir`) and the interpreter v0
   (`overt_interp::Interpreter`) evaluate compile-time calls, so `u8 := unsigned(8)` is computed
   from `lang.int`. Typed: namespace members against their annotations, function bodies
   (parameters, locals, names by `DeclType`, arithmetic, calls against `lang.Fn` signatures, the
   result against the declared one), `int.width_cap` on run-time intermediates. Next:
   - Operators: `interface(f)` evaluates to `Value::Interface(f)`; a non-arithmetic operator is
     typed by its function in the requirement struct `Call(Instance { f, [Type(left)] })`,
     through `MemberType` (decision 69, tables `OPERATORS`, `BORROWS`), so `a < b` is `bool`
     from `lang`. Next: operand checks against the requirement's parameters, `types.no_impl`
     (needs impls), prefix `~` and `-` through interfaces.
   - Conditions (decision 71): `Env::{form, condition, holds, decide, binary, unary}`
     (`env/condition.rs`) find `lang.bool` and its truth through `FORMS`; `Env::member` reads
     enum variants (`MemberTable::variants`, `Found::Variant`); the typer types `and`, `or`,
     `!`, `if` (`typeck/control.rs`, `TyKind::join`); MIR has `TerminatorKind::Branch` and
     lowers `if`, `and`, `or` (`build/control.rs`); the interpreter branches with `Env::holds`.
     Loops (decision 72): MIR lowers `for {}`, `for c {}`, labels, `break v`, `continue`,
     `return`, uninitialized locals and assignments to locals (`build/{loops,assign}.rs`);
     `x op= y` is `Rvalue::Narrow` with the place's type from `Typed` and the table `COMPOUND`;
     `Value::narrow`; the interpreter's `BUDGET` per call. Traps (decision 73): `trap::{Trap, Reason}` in `Unevaluated::Trapped(Box<Trap>)`, placed and traced by `Unevaluated::through(origin)` as they propagate (interpreter statements, `Env` calls); `Trap::report(root)`; reported by `Typer::ty` for annotations and by the checker, which computes every top-level value.
     Expected types (decision 74): `Typer::{typed_as, expected}` and `result`;
     `typeck/{block,expected,pattern}.rs`: statements and assignments, `.name` (`dot`), tuples,
     patterns against types (`pattern`, `test` through borrows), `match` (`matching`),
     `if p = e`; `Typer::join` for branches; `TyKind::{accepts, join}` item by item.
     Sequences (decision 75): `TyKind::Slice`, `Value::Array`, `Env::apply`, `TYPE_PREFIXES`;
     `typeck/sequence.rs` (tuples, arrays, repeats, integer indexing).
     Next: values of these forms at compile time (`Env` evaluating with an expected type, MIR
     aggregates and `match` lowering), exhaustiveness (decision 29), narrowing by comparisons
     (decision 35), `for p in e`, `#[comptime.fuel]`.
   - Struct declarations are typed in their instance (`MemberType`); bodies of generic functions are typed
     only where no `$` parameter is needed (`Typed(ExprRef)`), not per instance yet.
   - Impls (decision 76, `sema/src/witness/`): `Impls(FileRef)` collects `#[impl(I(T))]`
     declarations, reading `I` and `T` from the instance of the requirement struct `I(T)`;
     `T` may be a type constructor (`TyKind::made_by`), whose impl is a function of its
     arguments. `Witnesses((file, ty))` gives a type's witnesses over the file's package graph;
     `Env::method` finds a member of a witness, then a default of the interface, used by
     `Typer::member` (methods and `T.name`) and `Env::member`. The checker reports
     `impl.missing_requirement` and `impl.duplicate` (`check/impls.rs`). Next: the collision
     rule (a witness member named like `T`'s own members or another witness's), checking the
     types of provided members against the requirements, requirements of constructor impls per
     type, constrained `$W: Writer` parameters, `types.no_member` on values now that impls are
     collected, MIR and the interpreter calling methods, integer operators as `lang` impls.
   - Methods of built-in types are std impls over type constructors (the user's choice).
     Decision 77: `std.int.Narrow` (`lib/std/std/int.ov`) is the impl for `lang.Int` giving
     `to`, `wrap`, `sat`, `try_to` over `lang.int.{to, wrap, sat, fits}` (`Primitive::apply`,
     which now takes the `Env`). `eval::Func` is a function value with the call it was built in
     (`Env::within` binds outer calls first; `Env::inside` binds some parameters, for
     signatures); `MemberValue((ty, name))` evaluates a struct's declaration in its instance;
     `typeck/call.rs` types `$`-dependent signatures per call (`Typer::dependent`); `Env::call`
     evaluates `a.f(b)` as `T.f(a, b)` with `Value::ty`; `TypedBody::methods` lets MIR lower
     method calls (`build/method.rs`); `Rvalue::Comptime(expr)` (replacing `Nominal`) evaluates
     literals and outer `$` parameters in the frame's environment. Next: `abs` (its result type
     needs range bounds from `lang`, such as `lang.int.magnitude(r)`), `.wrap` only for
     power-of-two ranges (spec 04 says so; `Value::narrow` wraps any finite range), slice and
     array `.len`, a member of `[T]` read without a call (spec 05), not a method: decide how
     `[T]` declares it; pointer `.cast`.
   - Visibility (decision 70): `.workspace` is not enforced (`Env::sees` returns `None`) until
     `PackageData` records the workspace a package belongs to (the root manifest's `.members`).
     `.name` with an expected type is only implemented for `lang.scope` inside `#[visible]`;
     general expected-type lookup comes with enum literals and `.{ }`.
   - A call's argument diagnostic names the parameter type by its value (`Int(0..=255)`); label
     the parameter's declaration when the callee is a known function literal.
   - `lang.bool` and comparisons: comparisons desugar to `lang.ops.Eq`/`Ord`, whose results name
     `bool` in `lang`; find it by data, never by name. Then `if`, `and`, `or` in MIR (branches).
   - Appendix B as a `.ov` table (operators to `lang.ops` items), generated; MIR's operators are
     grammar `TokenKind`s until then.
   - `int.does_not_fit` in `corpus/reject/integers/does_not_fit.ov` (`p.x.abs() + p.y.abs()`)
     waits for `std.int`.
   - Persist the front-end queries (`Persist` needs `Encode`/`Decode` for their keys and values;
     `Symbol` and `OriginId` persist by content) and wire `.overt/queries/<toolchain hash>`.
2. Packaging, still open: signed binary caches (decision 63) and planned builds wait for the
   evaluator; later `--verify-reproducible`, `overt pkg why`. `lzma-rs` writes uncompressed xz.
   Paths in diagnostics mix relative and absolute forms depending on how a directory was found.
3. `names.private` (needs member types), `binding.hole_value` (needs attribute resolution).
   Spreads through aliases are forbidden by spec 01, section 8; spreading a `struct`
   declaration's members needs sema.
4. Fuzz the parser longer (120 s runs are clean).

`cargo xtask corpus` ends with the phase 1 progress measure (`overt_driver::Coverage`, `Session::coverage`): the share of function-body expressions sema types and of functions MIR lowers in `corpus/accept` (26% before decision 75, 32% after, 33% and 17 functions after 77). The corpus has 40 accept and 55 reject programs plus 63 explanation pages (`cargo xtask corpus`:
149 passed, 38 pending on later stages, 20 shown as transcripts, 0 failed), plus
`corpus/build.ov` (prelude `std.prelude` then `support`) and `corpus/support`, stand-ins for what
programs take from drivers, other libraries and unwritten parts of `std`.

## 9. Open questions and known gaps

- Decisions 65 (partial graphs resolve through the lockfile), 66 (primitives from `lang`), 68
  (`lang.Fn`, what is compile-time-only, nominal type names), 71 (conditions as data), 76 (impls over type constructors), 77 (function values in their call, `std.int`) and 69 (appendix B as data, `Ord`,
  `Eq.ne`) are proposed; 70 (visibility levels) is accepted; 67 (pure-Rust dependencies) is accepted. Decisions 45 to 64 are accepted. 61 (query persistence) and 63 (signed
  caches) are accepted but not built. Earlier: decisions 45 to 52 are accepted (50: BLAKE3 Merkle hashes, base32, nested store; 51: `.ovpkg`; 52: error chains) (fourth session): `lang.int`, spreads, compile-time block
  bindings visible in their own value, manifest detection and content-hash caches, `targets` as a
  default dependency. Corpus type functions write `$Ring`, `$Regs`, `$Block`.
- Build files name `std.build.Build` explicitly (no build-only prelude). New questions go there as
  "proposed" until the user decides.
- `lang/errors.ov` now declares planned ids for later stages (`names`, `binding`, `safety`,
  `place`, `borrow`, `panic` groups, `int.width_cap`), one per spec rule the reject corpus
  exercises. Their names are proposals; rename freely, then `cargo xtask generate`.
- Diagnostics have no explanation pages (`errors/*.md`) yet; `lang/errors.ov` references them.
- Whitepaper bibliography partly from memory; research notes' **[unverified]** claims stay
  unverified.
- `overt-tool`, `lir` and the MIR passes are stubs; `sema`, `mir` and `interp` are at v0.

## 10. Pitfalls met

- Error cascades: the lexer records the tokens it diagnosed (`Tokens::diagnosed`); the parser stays
  quiet right after consuming one, and skips same-line junk before a missing `;`. Keep new
  recovery generic in the same way.
- Python one-liners that open a file for writing before reading it truncate it. Heredoc Python
  with Rust string escapes is fragile; for edits containing backslashes prefer the Edit tool.
- `cargo fmt` silently leaves overlong lines inside long string literals; wrap them by hand.
- Generated files carry `#[rustfmt::skip]` on their `mod` declaration, so `cargo fmt` does not
  fight prettyplease; the staleness test compares exact bytes.
- `tracing::instrument` fails through the re-export; use `debug_span!` from `overt_trace::tracing`.
- In tests, `use TokenKind::*` shadows `String`; clippy pedantic also rejects enum glob imports
  outside tests, so alias (`use TokenKind as K;`).
- `'x` lexes as a label; `'\n` without a closing quote is the unterminated character case.
- `CLAUDE.md` is ignored by the user's global gitignore, so `jj st` never shows it.
- Node fields: each distinct shape is its own kind (`binding`/`typed_binding`,
  `struct_lit`/`typed_lit`); a field named after a node kind is restricted to it; `error` is a
  branch that wraps what recovery skipped, so a failed entry is one node. The fuzzer finds parser
  outputs that do not fit their slots: run `cargo +nightly fuzz run parse`, then `fuzz tmin`, then
  `overt dump cst` on the minimized input (dumps show each child's field).
- `cargo xtask generate` is built from the driver: when the driver uses a new diagnostic id,
  bootstrap with a temporary existing id, generate, then restore.
- A `?` inside a match arm returns from the whole function: in readers that must report every
  problem, combine results with a `match` on the tuple instead.
- Query values must hash by content: `HashMap` is not `Hash` (use `BTreeMap`), and `Arc<Hir>`-like
  values hash by the text they come from. Reports are data (`DiagCtxt` in a value, deduplicated by
  `absorb`); a query never emits.
- `DiagId::within` counts an id as within itself, so a stage may list a single id
  (`errors::INT_WIDTH_CAP`) as well as a group. Listing `int.does_not_fit` would fail
  `corpus/reject/integers/does_not_fit.ov` until methods (`.abs()`) are typed.
- Checks report only what they know: an `Unevaluated` value or an unknown type is silence, never a
  guess. A stage lists exactly the ids it implements in `stages::STAGES`, or corpus files it cannot
  handle yet fail instead of staying pending.
- Performance: one walk per purpose. Lowering fetches `Cst::fields` once per node; resolution
  and the syntactic checks share one walk; verifiers and dumps run only in tests and on demand.
- `OVERT_BLESS` blesses when the variable is present, whatever its value: `OVERT_BLESS=0` blesses,
  and so does an empty `OVERT_BLESS=` prefix. Never write it on a command unless blessing.
- Lib files are resolved too: a name used in `lang` must exist in `lang` (its prelude is
  `.{ "lang", "lang.int" }`, no `std`), so integer widths live in `lang.int`, not `std`.
- Every diagnostic helper phrase has one home: `Msg::did_you_mean`; the `.ov` extension is
  `overt_span::EXTENSION`; namespace paths on disk go through `overt_build::layout::Location`.
- Parallel work in a second jj workspace (`jj workspace add`) needs its own `CARGO_TARGET_DIR`:
  artifact names do not include the source path, so two trees sharing `~/.cargo/target` build
  each other's crates. Before rebasing a workspace's commits from another workspace, run `jj st`
  in it so its working copy is snapshotted; otherwise its edits end up in a divergent recovery
  commit.
- Tests never touch the user's store: the driver test fixture (`tests/common`) gives each session
  a store in its scratch directory, and the CLI honours `OVERT_STORE`.
- Test manifests are written with `Manifest::source`, never formatted by hand.
