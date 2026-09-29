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
- The toolchain file is read by every session that chooses a target, tests included (see
  `commands.md`). Its field errors reuse the `manifest.*` ids, as the lock file's do.

## Tests

- Snapshots are written only with `VUN_BLESS=1` (`0` or empty is off), and stale ones fail.
- Before committing sema or `std` changes, run `cargo nextest run -p vun-driver`: the corpus does
  not exercise `std.fmt.print`'s staged paths.
- `BackendCaps` is a `Feature` set; unimplemented backends are `vun_backend::Planned`;
  `Store::stage` returns a per-process `Staging` guard removed on drop; `xtask`, `vun-command`
  and `generate` declare tool-local diagnostic ids.

## Sema

- Checks report only what they know: unknown types and `Unevaluated` values are silence.
- Typing checks both branches of an `if` against the expected type, even when the condition is
  known: `x: Int(0..=0) = if true { 0 } else { 1 }` is `int.does_not_fit`.
- The parser's depth bound counts each link of a chain; past it the rest of the expression is one
  `Error` node, so later stages never recurse deeper than about twice `MAX_NESTING`.

## Writing Vun

- `or` is a keyword; `$if` cannot bind a pattern; child namespaces are not in scope by bare name
  inside their parent; private fields are visible only inside their struct literal; kind
  attributes go on the type's literal; inside a `lang` namespace file its own names shadow
  `lang`'s; integer arithmetic is exact (`x * 2` for `u32` does not fit `u32`); no operator may
  be written inside `lang`'s operator impls; attributes are bare only from the prelude's
  namespaces.
- A table in `desugar.vun` naming `lang` items is a tuple `.{ }`: its items have types of their
  own, and an array would not type. Borrow modes are lists of `BorrowWord` (`grammar.vun`).

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
