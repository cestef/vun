> Parent note (2026-10-07). This is the performance lane's report, filed verbatim below. Two
> additions the parent made after it returned: the P0 numbers were reproduced independently
> (`target/release/vun check lib/std --timings` against a fresh `VUN_STORE`: `stage save memos` own
> 1841 ms, shared file 17 688 422 bytes, project file 61 766 917 bytes, file sizes confirmed on
> disk), and the lane's "Checked, clean" line saying the 300-line limit holds is wrong: `cargo
> xtask lines` fails on `compiler/back/cranelift/src/tests.rs` (see `README.md`). The lane's `[P1]`
> for `StackUse::deepest` is reasoned, not measured; the parent read `analysis/stack.rs:106-119`
> and confirms the tail chain is walked once per member with a linear `reached.contains`.

## Performance

- [P0] A cold build's memo save encodes and writes ~79.5 MB (17.7 MB shared + 61.8 MB project) and costs ~1.7-1.9 s of a 5.8 s `vun check lib/std`; the project file is written whole, so the library memos it shares with the just-written shared file are encoded a second time - `compiler/base/query/src/persist/mod.rs:88` (save_keys), `:208` (`Keeps::of`), `:234` (`writer`), `compiler/base/query/src/persist/query.rs:150`, `compiler/tools/driver/src/queries.rs:120` (share), `:176` (save)
  Evidence: MEASURED `target/release/vun check lib/std --timings` (fresh `VUN_STORE`): `stage save memos own 1739 ms` over two runs (timeline: 698 ms + 1041 ms), total wall 5.8 s; `VUN_LOG=info` reports `shared queries saved bytes=17688422` and `queries saved bytes=61766917` (files confirmed at 17 688 422 and 61 766 917 bytes on disk). REASONED `Keeps::of(None, _)` returns `Nothing`, so `writer()` is `Writer::new()` and `save_keys` encodes every key into the project body, including the ones `share()` just wrote to the shared file; `values.write` (`query.rs:150`) re-encodes each value that a `Memo` already held in memory. The owner's list says "~37 ms for 4.3 MB" for the shared writer; the shared file alone is now 4x that size and ~19x that time, and it is half the total save.
  Cost: once per cold project (and whenever the body outgrows `GROWTH`), ~30% of a cold check/build; the shared subset (~17.7 MB) is encoded twice.
  Fix: after `share()` writes the shared file, attach it (`Db::load_over` / `Keeps::Base`) before `db.save()` so the project file is layered and its shared memos are not re-encoded; and keep the encoded bytes in `Memo` (or a `Vec<u8>` beside `known()`) so a save does not re-run `values.write` on values it already encoded. The existing `Writer::over` / `Keeps::Base` path already supports this.

- [P1] The witness lookup reads every file's `Impls` (and `Derives`) for every `(file, type)`: ~3882 computes each scanning ~265 file memos (~1M `cx.get` calls), 709 ms own, and each read is recorded and persisted as a dependency - `compiler/sema/core/src/witness/lookup.rs:61` (`files.iter().flat_map(|f| cx.get::<Impls>(f).to_vec())`), `:64` (Derives), `compiler/sema/core/src/witness/mod.rs:260` (`impl_files`)
  Evidence: MEASURED `--timings` row `witnesses own 709.36 ms total 1783.35 ms runs 3870` (12% of the cold check, ~183 µs/compute); `VUN_MEMOS=1` shows `impls absent 278` (one per file) and `witnesses absent 3882`. REASONED `impl_files` returns the whole dependency closure: for `lib/std` that is std (176 files) + lang (89) ~ 265, and `Witnesses::compute` reads `Impls` for each on every compute; at ~0.6 µs per get that is ~160 µs of the 183 µs own.
  Cost: superlinear, O(types x files) per build; every such read is a persisted `SavedDep`, so it also inflates the 61.8 MB store and the save above.
  Fix: memoize one `Impls` aggregate per package (or an index from an impl's target head symbol to its impls) and have `Witnesses::compute` read that single memo, then filter candidates; this removes the fanout from both the compute and the saved deps. `Impls` is already a per-file query, so an `ImplsOf(package)` wrapping it is small.

- [P1] `StackUse::deepest` is O(k^2) per root and O(k^3) over all roots for a chain or cycle of k tail-calling functions: tail-call targets are never marked done, and each is found by a linear `reached.contains` - `compiler/low/lir/src/analysis/stack.rs:106` (`deepest`), `:119` (`if !reached.contains(g)`), driven by `compiler/tools/driver/src/stack.rs:23`
  Evidence: REASONED (not measured) `for (_, inst) in unit.functions[h].insts { ... if !reached.contains(g) { reached.push(*g); } }` inside `while let Some(&h) = reached.get(next)`, and `visit` sets `state[f] = Done` only for the root, so every member of a tail chain/cycle is re-walked when `unit.functions.iter().map(visit)` reaches it. Sum over roots of O(k^2) is O(k^3).
  Cost: every `vun build` runs `StackUse::of`; the motivating workload (threaded interpreter / state machine, spec 09 section 3) has k mutually tail-calling handlers, so a large one pays O(k^3) with a linear scan per link.
  Fix: memoize the per-function tail-chain result, which is decomposable (`chain(f) = max(contribution(f), max_g chain(g))`), by recording the suffix value for each node as the walk proceeds and setting `state`; replace the `Vec` + `contains` with an `IndexVec<FuncId, bool>`/`Set`. The allowed tail-call cycle is already stopped by `reached.contains`, so an `Open` marker covers it.

- [P2] `Concrete::propagate` reaches its fixed point one constant level per iteration: `constants()` is recomputed each pass and only recognizes a local written as `Use(Const)`, so a chain of k constant-dependent locals costs O(k) full-body scans - `compiler/mid/mir/src/concrete/mod.rs:136` (`propagate`), `:138` (`let mut constants = self.constants()`), `:156` (`if !self.fold(env) && !changed`)
  Evidence: REASONED `fold` rewrites `b = a + 1` to `b = 2` only after the operand pass already used the previous `constants`; the next loop's `constants()` then sees `b`, so `c = b + 1` folds one iteration later. A depth-k chain of `$`-generated arithmetic does k passes, each calling `constants()` and `fold` over every statement.
  Cost: per body in `concrete` (2586 computes, own 408 ms, longest 82 ms); superlinear for generated/macro-expanded bodies with long constant chains.
  Fix: a worklist over writes (revisit only users of a local whose constant changed), or have `fold`/the operand pass also set `constants[stmt.place.local]` when it folds a whole-local `Use(Const)`, so a single pass reaches the fixed point.

- [P2] `Move::sequence` orders parallel copies with `pending.iter().position(|m| !read(&pending, m.to))` plus `pending.remove(i)`, while `read` itself scans `pending`: O(k^3) for a jump with k parallel moves - `compiler/back/regalloc/src/resolve.rs:145` (`sequence`), `:150` (`position`), `:151` (`remove`)
  Evidence: REASONED `let read = |pending, at| pending.iter().any(|m| m.from == Source::At(at));` is called for each candidate inside `position`, and `Vec::remove(i)` shifts the tail.
  Cost: per block edge whose target has many parameters (SSA phi moves), e.g. generated code; k is the parameter count.
  Fix: Kahn's algorithm with an index from `Loc` to the move writing it and a per-destination read count, or swap_remove plus a destination map, so each move is examined O(1) times.

- [P2] `Table::entries()` re-hashes every key to test canonicity, and it is iterated by `number()`, `save()`, and `attach()`: each persisted key is hashed several times per save and per load - `compiler/base/query/src/table/mod.rs:167` (`entries`), used at `compiler/base/query/src/persist/query.rs:113` (`number`) and `:150` (`save`), `compiler/base/query/src/table/mod.rs:143` (`attach`)
  Evidence: REASONED `entry.at.get().is_some() || entry.key.get().is_none_or(|key| slots.get(&Probe::of(&**key)) == Some(slot))`; on a cold save every key has no `at`, so all ~86k+ keys of the persisted queries are re-hashed (keys are whole `Instance`s: a `Func` plus a `Vec<Value>`).
  Cost: part of the 1.7 s save and the 248 ms load; grows with the number and size of keys.
  Fix: store the hash in `Entry` when the key is inserted (it is already computed by `Probe::of`) and compare it in `entries()` instead of re-hashing.

- [P2] The lint check describes each node once per lint instead of once per node - `compiler/sema/lint/src/rule.rs:143` (`run`), `:150` (`for node in nodes.iter().filter(...)`), `:157` (`env.describe`)
  Evidence: REASONED `env.describe(shape, std::slice::from_ref(node), facts)` is inside the per-lint loop, so the same node's `Value::Struct` is rebuilt for every visible lint, each `name` field via `Value::string` (an `Array` of `Int`s); the descriptor does not depend on the lint.
  Cost: L x N describes per file (L = visible lints, N = nodes; `linted` is 178 runs, 2.08 s total, 99.7 ms longest).
  Fix: build the `Value` for each node kind once before the lint loop (keyed by node index) and index it, since only the lint's `check` and `allowed` filter differ per lint.

- [P2] The persist save scans `keys.replaced` linearly for every key left to the shared file - `compiler/base/query/src/persist/query.rs:178` (`loaded.is_some_and(|keys| keys.replaced.contains(&number.at))`)
  Evidence: REASONED `replaced` is a `Vec<u32>` and the check runs for each `number.at < first`; in a layered save with many recomputed shared keys this is O(keys x replaced).
  Cost: layered saves (the common warm path once the shared file exists).
  Fix: keep `replaced` as a `Set<u32>` or a `Vec<bool>` indexed by key position for the membership test (it is already deduplicated when built).

## Checked, clean

- In-flight tail-call change (`jj diff`): the new `low/lir/src/analysis/taken.rs` walks all instructions once per unit and is called once (`cranelift/src/tail.rs:57`); `low/lower/src/func/tail.rs` and the wasm `abi/mod.rs`/`call.rs` add only arity-bounded per-call work; `Tails::of` is one pass over reachable functions' instructions. The only new superlinear risk found is the stack-use walk above, which came with the same feature.
- Query engine: `Table::slot`/`Probe` hashes once per lookup and stores the hash (`hashed.rs`), `Cx::once` dedups reads by comparison up to `SCANNED = 32` then by set, and `fetch`/`verified` avoid recompute when a memo is current. No correctness-driven miss found beyond the witness fanout.
- `Body::prune` (reachability), LIR `sweep` fixed point, `StackUse` plain-call memoization, `Program::bodies` dedup, and `Assembly` id placement are all single-pass or map-keyed; the `Places`/`SiteIds` placement is once per site.
- Persist I/O: one `path.put` per file (no per-key writes/fsync); `Blob` reads in place, and `Writer::encode_with` swaps buffers rather than copying enclosing bytes. The cost is encoding, not I/O.
- wasm linker `Symbols::of`/`Layout::value` linear searches and `native`/`regalloc` `reg_of`/`Allocation::at` scans are over small collections (sections/imports/pieces per value), not the whole program.
- `PackageData::children`/`holds` rebuild and sort a package's file list per call, but `Declared` is memoized and its own time is 6 ms, so it is not a build cost today.
- MIR verifier (`verified()`), LIR verifier, and tokens/CST verifiers are `cfg!(debug_assertions)`-gated and do not run in release.
- `cargo xtask lines` (300-line limit) and the module split hold; no monolithic hot file found.

## Open questions

1. Is `impl_files` returning the whole dependency closure intentional for visibility, or can `Witnesses` be keyed by package (not file) so the O(types x files) fanout drops to O(types)?
2. Is the project query file meant to stay a self-contained copy of the library memos (the owner's "keep them in their own files"), or is layering it over the shared file acceptable? That decides the ~17.7 MB double-encode fix.
3. How large are real tail-call dispatch groups in the corpus? That decides whether the `StackUse` walk is P1 or P2 in practice; I could not build a chain program to measure within the time budget.
4. The witness and `typed`/`mir` memo keys include a `FileRef`, so a type seen from N files is computed N times; is that required for visibility, or can the key be the package plus a visibility scope?
5. Is `save` expected to run on `vun check` (not just builds), since it is 30% of the cold `check lib/std` cost?
