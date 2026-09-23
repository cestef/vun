# Handoff

State of Overt as of 2026-09-23 (fourth session), written for the next agent. Read this, then
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
its package and report `names.unresolved`. `overt check <file>` runs every stage;
`overt dump hir` prints the resolved HIR with origins. Every file in `lib/` and `corpus/accept`
parses, lowers, verifies and resolves cleanly. Nothing is type-checked yet.

History (jj, newest last): scaffold; lexer; parser, CST, tables, CLI and corpus; bitwise compound
assignments; corpus grown to 40 accept and 50 reject; modules split and the 300-line limit;
tagged literals and node fields; terminal renderer, `--format` and parser fixes; manifests;
ariadne renderer; handoff; settled open questions (decisions 28 to 44); HIR; name resolution; integer widths in `lang.int`; package loading, spreads and
`names.unresolved`; `overt-suggest` and `overt-hash`; graph, fs and outline crates; BLAKE3 Merkle
hashes and the nested store; `overt pkg fetch`, `.ovpkg` and error chains; file and directory
duplicates, `overt_fs::Edit` and `Scratch`; `overt pkg add` and `pack`, literals written back
(`Literal::source`, `laid_out`, `Decl::insert`, `Edit::apply`); the executable bit in snapshots,
tree hashes and `.ovpkg` 2.
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
lib/lang/lang/syntax/grammar.ov   THE grammar: token kinds, node kinds, enums, operator tables
lib/lang/lang/errors.ov           every compiler diagnostic id
corpus/accept             programs that must compile (CST snapshots in compiler/front/syntax/tests)
corpus/reject             must-fail programs with `//~ id` annotations (`cargo xtask corpus`)
fuzz/                     cargo-fuzz crate (outside the workspace): targets `lex`, `parse`
```

Generated files (never edit; edit the `.ov` source and run `cargo xtask generate`):
`compiler/front/grammar/src/generated.rs` and `compiler/base/diag/src/errors.rs`.

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
| `overt-span` | `FileId`, `ByteRange`, `Span`, `SourceFile` (`line_col`), `SourceMap`, `SourceFiles`. |
| `overt-origin` | `OriginId`, `Origin`, `Via`, `OriginTable`. |
| `overt-diag` | `DiagId`, `Severity`, `Msg` (typed parts, `append`), `Label`, `Fix`, `ChildKind` (Display), `Diagnostic`, `DiagCtxt`, `DiagCx::error_at`, `DiagSink`; `Msg::did_you_mean`; `errors` (generated). |
| `overt-suggest` | `closest` (did-you-mean by edit distance), `Lookup` on slices (find by name or the closest name). Generic, no diag types. |
| `overt-hash` | BLAKE3 only. `Algorithm` (typed, `name`), `Hash` (`blake3:<base32>`, `parse`, `digest`, `of(bytes)`, `tree(dir, include)`), `Snapshot` (per-file `Entry { hash, mode }` by path, `root` = Merkle tree over path components with directory nodes in a derived key, `changes`, text form, portable plain-component paths), `Change`. Package identity, manifest `.hash`, every cache key. |
| `overt-pkg` | `Mirror` (`parse`, `fetch`), `SCHEMES` (https, http, file), `FORMATS` (`.ovpkg`, `.tar.zst`, `.tar.xz`, `.tar.gz`, `.tar`, `.zip`; `strip` of a forge's top directory), `Archive` (`.ovpkg`: `pack`, `unpack` verifying each file), `Failure`/`Malformed` with sources. |
| `overt-graph` | `Requirements<K>` (`add`, `order`, `OrderError`): the pass manager's ordering. |
| `overt-fs` | `Tree` on `Path`: `files` (recursive, sorted), `entries`. `Modes` on `Path` (`mode`, `set_mode`: `Mode::{Regular, Executable}`); `Relative` (`relative_to`). `Edit` on `Path`: `put` (creates parents), `remove` (absent is removed), `fresh` (empty directory), `replace_with` (move a directory in whole). `Scratch`: a test directory removed on drop. Every filesystem write in the compiler and its tests goes through them. |
| `overt-outline` | `Outline` + `Outlined::node`: indented tree text for the CST and HIR dumps. |
| `overt-grammar` | `generated.rs`: `NodeField`, `NodeSlot`, `Count`, `NodeKind::slots` (children per kind, from tagged entries), `TokenKind` (+ `TRIVIA`, `ATOMS`, `KEYWORDS`, `PUNCTUATION`, `END`, `text()`), `NodeKind` (`LEAVES`, `BRANCHES`), `Context`, `Assoc`, tables `CONTEXTUAL`, `DELIMITERS`, `INFIX`, `PREFIX`, `POSTFIX`, `ASSIGN`, `OPERANDS`, `ELEMENT_TYPES`, `LITERAL_TYPES`. `lib.rs` derives lookups: `punctuation`, `keyword`, `is_trivia`, `infix` (`Prec`, `Infix`), `closer`/`opens`/`closes`, `is_leaf`, `contextual`. |
| `overt-syntax` | `tokens` (SoA tokens + trivia side table, `verify`); `lexer` (`lex`, recovery, `literal(kind, text)` for string values, backslash escapes); `cst` (postorder `(kind, Anchor, size)`, `Anchor::{Token, Missing}`, `children`, `span`, `dump`, `verify` -> `Malformed`); `parser` (`parse`); `literal` (`declarations` -> `Decl { name, doc, value: Literal, span }`, `Literal::Tagged`); `literal::{print, insert}` write values back (`Literal::source`, `Literal::laid_out` within `WIDTH`, `Decl::insert` -> `overt_diag::Edit`); `Cst::fields(n)` -> `Fields` (`get`, `one`, `assigned`), matched against the kind's slots; `children_rev` walks without allocating. |
| `overt-hir` | `tree` (arenas: namespaces, decls, blocks, exprs, pats, binders; `List`/`Entry::Staged` for `$for`/`$if`), `lower` (one `Cst::fields` per node), `visit` (`Visitor` + `walk_*`), `verify` (tree property, origins), `resolve` (scopes: namespace, local, function, signature; `Res::{Binder, Outer { name, ns }}`, `Resolution::parents`; the checks; `Resolution::check_outer` over the `Outside` trait reports `names.unresolved`), `dump`. `Member::Spread` (`..path`, `..path.{ a, b }`; selected names are binders). `Hir::members`, `Hir::binders`, `Hir::path`, `tree::each_entry`. |
| `overt-command` | Options of a group apply to every command below it; `Program.configure` applies them before a run. Declarative command trees: `Program`, `Command`, `Action::{Run, Group, Planned}`, `Param`/`ParamKind::{Positional, Flag, Option}`, `Args`, `parse` -> `Request`, `Selected`, generated `help`, `run` -> `Outcome`/`Refusal`, `Outcome` -> `ExitCode` (usage = 64). Usage errors are diagnostics with suggestions. |
| `overt-driver` | `stages::STAGES` (implemented stages and the diagnostic groups they report), `Session::check` (runs them), `commands::PROGRAM` (the whole `overt` tree of spec 12.7, most commands `Planned`; `dump tokens|cst|manifest`, `components` run), `formats::FORMATS` (renderer registry behind `--format`), `Registry`, `Session` (`load`, `read`, `text`, `lex`, `parse`, `lower`, `resolve`, `declarations`, `manifest`, `emit`, `failed`). `packages`: `PackageKey::{Content(Hash), Loose}`, `Package` (deps, prelude), `Session::{package_of, package_at, dep, namespace_at, members, exports, scope, spreads}`, `Scope`/`Outer`, `FileScope` (implements `Outside`), `Session::fetch` -> `Fetched` (graph walk, mirrors in order, staging, verify, commit) behind `overt pkg fetch`, `LIBRARY` (the toolchain library, today the repo's `lib/`). Caches keyed by content hash: manifests and file declarations by text hash, packages by include-tree hash. |
| `overt-cli` | `main` runs `PROGRAM` through `overt-command` and renders diagnostics once at the end. |
| `overt-render-short` | `file:line:col: severity[id]: message`, then `  = help: ...` lines (fixes too). |
| `overt-render-annotate` | default `terminal` format: labels by file, notes, fixes as patches, traces. |
| `overt-render-ariadne` | `--format ariadne`: one color per label role; fixes as help lines. |
| `overt-build` | `lock` (`Lock`, `Locked`, `FILE`, text form), `Manifest::{literal, source, identity}`, `Identity`, `EDITION`; `manifest::read` -> `Manifest` (`Version`, `Dep` with span, `Source`, `DepKind`, `prelude_span`), `manifest::declared` (a `build.ov` is a manifest iff it declares `package`), `DEFAULT_DEPS`, `DEFAULT_PRELUDE`; generic record reader reporting missing, unknown (did-you-mean) and ill-typed fields. `store::Store` (`.overt/store/<algorithm>/<2 chars>/<rest>/{package/,snapshot}`, `staging/`). `layout`: `MANIFEST`, `SEPARATOR` (from the grammar's `.`), `Location` (namespace on disk: `root`, `member`, `file`, `exists`, `children`, `path_of`). |
| `overt-harness` | `repo_root`, `expectations` (`//~ id [@col] ["fragment"]...` -> `Expected`, or `Malformed`), `Reported`, `compare` -> `Mismatch { missing, unexpected, differing }`, `discover`, `snapshot`, `snapshot_tree` (mirrors a source tree, `OVERT_BLESS`). |
| `overt-xtask` | `cargo xtask generate [--check]`: pluggable `generate::Generator { source, output, emit }` table (`grammar`, `errors`); shared read/parse/emit/format (quote + prettyplease). `cargo xtask corpus`: runs `corpus/accept` (must be clean) and `corpus/reject` (exact `//~` set) through `Session::check`; a file whose missing ids all belong to stages that do not exist yet is pending (`overt_driver::stages::STAGES`, groups from `errors.ov` namespaces via `DiagId::within`). |
| backends, linkers, target, pass, trace, tool | as in the first session (stubs with `const INFO`). |

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

Tests: 46, including unit tests per crate, `tests/lexer.rs`, `tests/parser.rs` (all of `lib/` and
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

1. Done in the fourth session: package loading, spreads, `names.unresolved`. Still open there:
   spread paths through aliases (`..uart;` after `uart := a.b;`) and through declarations that are
   not files (`..lang.int.X` where `X` is a `struct`); member paths (`a.b.c` checked against
   namespaces) belong with sema. A name declared in `x.ov` and held in `x/` is `names.duplicate`
   (`Session::check_children`).
2. Packaging roadmap from the user's Nix review, in order: a shared user store
   (`~/.cache/overt/store`) with project lockfiles as GC roots and `overt cache gc`; readable store
   entries (the hash stays the key); planned builds (typed step records, `overt build --dry-run`,
   steps keyed by inputs); signed binary caches (`.caches`, ed25519 trusted keys). Later: early
   cutoff, sandboxed steps, `--verify-reproducible`, `overt pkg why`, a `.replace` override.
   Done: minimal version selection (`Session::select`, `dep()` binds the selected version) and
   `overt.lock` (`overt_build::lock`), written by `pkg fetch` and `pkg add`; fetching, verification,
   `overt pkg add` (trust on first use, entries written through `Dep::declare`/`Decl::insert`) and
   `overt pkg pack`. Open: `overt pkg update` needs a source of newer versions (an index).
3. `names.private` (needs member types: sema), `binding.hole_value` (needs attribute resolution).
4. Desugaring into `lang` items with `Via::Desugar` origins (appendix B), queries, sema (with
   narrowing, decision 35, exhaustive match, decision 29), MIR, interpreter.
5. Explanation pages `errors/<group>/<id>.md` for every diagnostic, and `overt explain`.
6. Fuzz the parser longer (the verifier checks node fields; 150 s runs are clean).

The corpus has 40 accept and 54 reject programs (67 pass, 27 pending on later stages), plus
`corpus/build.ov` (prelude `std.prelude` then `support`) and `corpus/support`, stand-ins for what
programs take from drivers, other libraries and unwritten parts of `std`.

## 9. Open questions and known gaps

- Decisions 45 to 52 are accepted (50: BLAKE3 Merkle hashes, base32, nested store; 51: `.ovpkg`; 52: error chains) (fourth session): `lang.int`, spreads, compile-time block
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
- `overt-tool`, `overt-query`, `sema`, `mir`, `lir` are stubs.

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
- Performance: one walk per purpose. Lowering fetches `Cst::fields` once per node; resolution
  and the syntactic checks share one walk; verifiers and dumps run only in tests and on demand.
- `OVERT_BLESS` blesses when the variable is present, whatever its value: `OVERT_BLESS=0` blesses.
- Lib files are resolved too: a name used in `lang` must exist in `lang` (its prelude is
  `.{ "lang", "lang.int" }`, no `std`), so integer widths live in `lang.int`, not `std`.
- Every diagnostic helper phrase has one home: `Msg::did_you_mean`; the `.ov` extension is
  `overt_span::EXTENSION`; namespace paths on disk go through `overt_build::layout::Location`.
