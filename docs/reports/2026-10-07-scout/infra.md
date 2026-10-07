## Base infrastructure, object formats and linking

- [P2] A declaration's anchored identity silently truncates past 65535 same-named siblings, panicking on valid input - compiler/base/origin/src/lib.rs:66 (Anchor::Declaration { .., nth: u16 }), reached from compiler/front/hir/src/lower/mod.rs:60 and :121
  Evidence: `named: foldhash::HashMap<(Option<OriginId>, Symbol), u16>` (hir/lower/mod.rs:60), then
  `let nth = *named; *named += 1; self.origins.declaration(span, self.declaration, name, nth)` (hir/lower/mod.rs:121),
  and `Anchor::Declaration { within: Option<OriginId>, name: Symbol, nth: u16 }` (origin/lib.rs:66). `OriginTable::declaration` then asserts the identity is unique: `assert_eq!(self.origins[*earlier as usize].span, span, "two declarations share an identity")`.
  Why: a module with 65536 declarations of one name (e.g. 65536 `fn f` at top level, key `(None, f)`) wraps `nth` to 0 in release (or panics on the `+=` in debug), so two distinct declarations get the same `(within, name, nth)` identity with different spans and the `assert_eq!` fires. The compiler panics on a valid program; the limit is unnamed and undocumented.
  Fix: make `nth` `u32` (matching `OriginId`/slot widths) and derive it through a named conversion, or have `declaration` take the ordinal as `u32` so the identity space cannot silently wrap. (Cross-area: the u16 lives in `origin`, the counter in `hir`.)

- [P2] The saved-dependency read swallows its decode error with `.ok()?`, unlike every sibling read path - compiler/base/query/src/memo.rs, `Deps::get`
  Evidence: `let saved = store.stream.read_blob::<Vec<SavedDep>>(*at).ok()?;` drops the `Malformed` and returns `None`, which becomes `Why::Unresolved`; contrast `Memo::value` (`Err(malformed) => tracing::debug!(query = Q::NAME, %malformed, "saved value not read")`) and `Attached::key` (`inspect_err(|malformed| tracing::debug!(query = Q::NAME, %malformed, "saved key not read"))`.
  Why: a corrupted cache blob is indistinguishable from a dependency that is merely not held, and neither the recomputation nor the `changed` log names the malformation. Debugging "why did this recompute" loses the one datum that says the cache was corrupt; the codebase's own convention is to log the malformation at `debug`.
  Fix: `inspect_err(|malformed| tracing::debug!(query = Q::NAME, %malformed, "saved reads not read"))` before `.ok()?`, keeping the `None` return so behaviour is unchanged.

- [P2] The verify-ahead record grows one entry per revision and is never pruned - compiler/base/query/src/cx/settle.rs (`holds`, `self.db.settled.pin().insert(at, found)`) and compiler/base/query/src/db.rs (`settled: Map<(TableId, Slot, Revision), Found>`)
  Evidence: `let at = (table.id, slot, self.revision);` then, on the non-`Unsure` path, `self.db.settled.pin().insert(at, found);`; the only removal is `if found == Found::Unsure { … remove(&at) }`. `verify.rs` reads it through `settled_in((table, slot), revision)`, which only ever asks for the current revision.
  Why: entries for superseded revisions are dead (only `Found::Holds` for the current revision is read) but retained. A long-lived `Db` across many edits accumulates `(table, slot, revision)` rows monotonically. Negligible for a one-shot compile, a real leak for a reused database.
  Fix: key the map by `(TableId, Slot)` with the revision stored in the value and drop non-current entries on revision bump, or clear it in `set_as` when a new revision starts.

## Checked, clean

- Query engine memo/cache identity: `Fingerprint` (128-bit `Stable::wide`), `Hashed`/`Probe` (hash equality plus `Eq`), `Dep` lists interned by value in `Db::list`, save keys found by `Stable::of(key)` then confirmed with `self.key(at)? == *key` (`Attached::find`). Base32 digest decode is length- and trailing-bit-strict (`hash/src/base32.rs`), so `Hash::parse` cannot accept a truncated digest. `Build::identity` keys saved memos by executable path/size/mtime as documented.
- Query memo/settling logic read in full: `db.rs`, `memo.rs`, `claim.rs`, `cx/{mod,verify,settle,reuse,stored,frame,guard,all,cycle,why}.rs`, `persist/{mod,load,store,saved}.rs`. Cycle detection (`compute`/`closed`/`Standing::Provisional`) and the wait-graph cycle break in `claim.rs` are internally consistent.
- `VUN_QUERY_CHECK` (`Reuse::Check` → `Cx::check`) is wired exactly where reuse skips a computation and panics with a typed `Unsound` when they disagree; no path reuses a memo without going through `verified`/`current`/`durable`.
- `vun-par` pool soundness argument holds: a worker is counted on the call latch under the pool lock inside `State::join`, `Lent::until_done` closes the call under that same lock before waiting, and `Work` is only called by a counted worker before it reports. No unsynchronised window found.
- `vun-component` `unsafe` matches decision 498's written argument (`abi.rs`, `host.rs`, `guest.rs`): the descriptor is validated on `ABI` before use, buffers are freed by their allocator, and `Owned::new`/`into_vec`/`copied` handle the null/empty cases.
- Diagnostics keep context: `Diagnostic::caused_by` is the single sanctioned `to_string()` flatten point and walks `Error::source()`; `Cause`, `Fault`, `Unloaded`, `Tool` errors implement `source()`; `DiagCtxt::{absorb, absorb_through, reached_once, sorted}` are order-independent.
- Object formats: `uf2` (`block.rs`, `lib.rs`) round-trip and bounds (`size` vs `ROOM`, magics, numbering) verified; `wasm` `read/{mod,entries}` section/reloc bounds checks (`checked`) look correct; `core/src/machine.rs` (Thumb bit, REL addends, pointer width) consistent. No `unsafe` in `object/` or `link/`.
- `hash/src/{lib,stable,tree}.rs` (tree node kinds, `parse`'s file-vs-directory guard, the `Hasher` gather buffer), `span/src/{lib,file,key}.rs`, `fs/src/edit.rs`, `origin` identity/`Resolve` anchors, and the two `unreachable!()` sites (`data/src/json/parse.rs:164`, `derive-core/src/item.rs:112`) — all genuinely unreachable; no `.unwrap()` outside tests across `base/`, `object/`, `link/`.

## Open questions

1. Is `Anchor::Declaration { nth: u16 }` an intentional budget with a documented declaration-count limit elsewhere, or should it widen to `u32`?
2. Is a single `Db` intended to live across many user edits (LSP/long-lived `vun`), which would make the unpruned `Db::settled` growth actionable?
3. Should a malformed saved-deps blob be distinguishable from an unresolved dependency in `changed`/tally output, or is silent recomputation the intended policy?
4. `cargo xtask lines` was not run; I spot-checked the largest files in `base/query` (all <300) but did not exhaustively confirm the 300-line limit across all of `base/`, `object/`, `link/`. Should I be given a run for that?