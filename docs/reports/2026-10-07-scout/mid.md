## MIR and its passes

- [P1] `Remap` renumbers every place a rvalue holds except an inline-asm block's output places, so `expand` inlines a callee whose asm writes to the wrong caller locals - `compiler/mid/mir/src/remap.rs:31` (with `compiler/mid/mir/src/build/expand.rs:89`, `compiler/mid/mir/src/rvalue.rs:150`)
  Evidence: `Remap::rvalue` remaps operands plus only `Ref`/`IsVariant`/`Resume`/`End` places:
  ```rust
  pub(crate) fn rvalue(&self, value: &mut Rvalue) {
      for operand in value.operands_mut() { self.operand(operand); }
      if let Rvalue::Ref { place, .. } | Rvalue::IsVariant { place, .. }
      | Rvalue::Resume { frame: place, .. } | Rvalue::End { frame: place } = value { self.place(place); }
  }
  ```
  and `Rvalue::operands_mut` for `Asm` yields reads only (`Self::Asm(asm) => asm.reads_mut().map(vun_inline::Read::read).collect()`, rvalue.rs:150), never the `Out`/`InOut` places from `asm.outputs()` (`rvalue.rs:191`). `expand` clones the callee body, pushes its locals after the caller's (`n0 >= 1`, the result local), and calls `remap.stmt(&mut stmt)` on each statement before assigning it (`expand.rs:89`), so an asm output place keeps the callee's local index `j` instead of `n0 + j`.
  Why: a function with a `Code(T)` parameter whose body contains inline asm writing an output, called with code (the only path that reaches `expand`), inlines its asm with outputs still pointing at the callee's local numbers. `j < L <= n0 + L`, so the index is always in range and the verifier accepts it: the asm silently writes an unrelated caller local, with no diagnostic and no verifier catch. `Body::constants` (concrete/mod.rs:184) and the place-state pass (places/transfer.rs:46) both treat asm outputs as ordinary places, which is exactly what the remap must also do.
  Fix: in `Remap`, rebind the asm bindings, not just reads - iterate `asm.bindings_mut()` (the idiom `low/passes/src/outline/mod.rs:164` already uses) and remap each `Bound::Out`/`Bound::InOut` place through `self.place`, alongside the existing reads. A debug verifier rule cannot see this (indices stay in range); a round-trip test that expands a body containing asm and asserts the output place maps to `n0 + j` would.

- [P2] Coverage counts divide by `lang.cover.size` with no zero guard - `compiler/mid/mir/src/concrete/covered.rs:56`
  Evidence:
  ```rust
  pub fn number(i: &Instantiation, index: usize, size: u64) -> u64 {
      Stable::of(&(i, index)) % size
  }
  ```
  `Map::of` accepts any `Value::Int` that fits `u64` (covered.rs:37), and `Region::of` passes that straight to `number` (covered.rs:72). A `lang` whose `cover.size` is 0 divides by zero and panics while lowering a covered build.
  Why: `Region::of` is reached whenever `Coverage::Blocks` is on; a `lang.cover.size = 0` (mis-declared table) panics the compiler instead of reporting a bad table.
  Fix: return `None` from `Map::of` when `size == 0`, or `checked_rem` in `number`, so a zero size is treated as "no coverage" rather than a crash.

- [P2] Two declared public modules are empty scaffolding - `compiler/mid/passes/src/coroutines.rs:1`, `compiler/mid/passes/src/ranges.rs:1` (declared at `compiler/mid/passes/src/lib.rs:4`, `:9`)
  Evidence: both files are a single doc line and nothing else (`//! Coroutine frame layout and lowering (spec 06).`, `//! Range analysis and check elision.`). The functionality they name does not live here: coroutine frame lowering is in `compiler/low/lower/src/func/coroutine.rs` and `compiler/mid/interp/src/frames.rs`, and range check elision is in `compiler/mid/passes/src/panics.rs`.
  Why: `lib.rs` advertises two public modules that hold no code, so a reader looks for coroutine lowering or range elision in `mid/passes` and finds nothing; if this is a reserved placeholder it should at least say so.
  Fix: delete the two modules and their `pub mod` lines, or make them re-export the code that actually implements the named concern. (If they are intentional placeholders, ignore.)

## Checked, clean

- `mir/src/verify.rs`: `Body::check` covers locals/blocks existence, distinct params, per-entity origins, empty body, entry-targeting, yield resume/cancel distinction, `Ended` only when a body yields, and past-a-non-returning-asm only as the last statement with `Unreachable`. `rvalue`/`place`/`operand` walk every place-bearing rvalue including asm `outputs()`. No gap found beyond the remap issue above (which the verifier cannot detect by construction).
- `mir/src/build/mod.rs` `Body::prune`/`Body::constructor`, `build/{control,loops,suspend,staged,pattern,assign,temp,aggregate,call,value,closure,early_return,asm}.rs`: lowering assigns an origin at every statement and terminator; coroutine `suspend` allocates a fresh `resume`/`cancel` per yield so `Body::cancel_of` (interp/frames.rs:53) has no ambiguous match; `defer.rs` captures `consumes` before later blocks exist, so the list is not polluted by following code.
- `passes/src/places/{mod,transfer,states,parts,moved,leak}.rs`: forward solve then re-run for reporting; `Defer` run/skip decision and ambiguous-join report; linear-part tracking; move-out-of-borrow detection via `Place::types` prefix pairing (projection `i` paired with the type *before* it, which is the container) is correct.
- `passes/src/borrows/{mod,flow,access,later,free}.rs` and `panics.rs`: loan construction, liveness, conflict/escape/outlived reporting, `later_use` search, free-value report, and `MayTrap` cycle handling (`cycle => true`) all match their docs; `indices` bound check via `place.types` is correctly aligned.
- `mir/src/concrete/mod.rs`: `constants` counts borrow/asm writes so a local a borrow could write is never propagated as constant; `propagate` zips `self.params` with `bound`, and `self.params` includes the comptime-param locals (built per HIR `Fn::params`), so the pairing is sound.
- `Fu-collect`, `Program::bodies` dedup, and `reach.rs` value/fn collection: no duplicated-list or stringly-key issue found.

## Open questions

- Is the empty `coroutines.rs`/`ranges.rs` a deliberate placeholder (so this is a no-op), or leftover scaffolding to remove?
- Is `expand`/`Remap` actually reachable with an asm block in practice (a `Code(T)`-parameter function containing inline asm)? That fixes the severity of finding 1; the mechanism is proven from code, the trigger shape was not compiled here.
- Should the MIR verifier reject an asm output place whose local lies outside the body's own locals only, or is a structural "asm outputs are remapped exactly like operands" invariant test wanted instead?