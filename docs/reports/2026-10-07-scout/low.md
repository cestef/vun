## LIR, lowering, legalization, ABI

The in-flight guaranteed-tail-call work (lower → verify → legalize → backends) is internally consistent on the paths I could trace: `lower/func/tail.rs` sets `landing = self.out` and appends it last, `verify/calls.rs::replaces` requires exactly `args.last() == out` for a through result, and `legalize/rewrite/{call,term}.rs` threads `self.result` through with `Parts::Written` → `Return(None)`. I found no wrong-output/crash bug with a proven victim; the findings below are gaps and doc/code drift.

- [P2] `docs/open/language.md` names a removed enum variant and claims a just-added feature is missing - docs/open/language.md:13
  Evidence: the line reads "a wide integer result (`Unlegalized::TailResult`)", but `Unlegalized::TailResult` was deleted in this same change (`compiler/low/passes/src/legalize/mod.rs` diff removes the variant) and wide-result tail calls are now legalized (`compiler/low/passes/src/legalize/rewrite/call.rs:37-41` pushes `self.result` instead of `Err(Unlegalized::TailResult)`; new test `a_tail_call_of_a_wide_result_writes_through_the_address_received`, and `tail_memory.vun`'s `widen` runs it a million deep). `rg TailResult` matches only this doc line.
  Why: the "what is open" tracker agents read now tells them a variant that does not exist and a feature that does exist are both the opposite of reality; a reader would re-add the error or skip work.
  Fix: drop the "wide integer result (`Unlegalized::TailResult`)" item and keep the genuine remaining lacks (Indirect record on arm64/Windows, C record result).

- [P2] The user-facing `Form::Tail` message omits one of the two causes its own doc names - compiler/low/lower/src/unlowered.rs:70-72 vs the `what()` arm (~line 211)
  Evidence: the variant doc is "A tail call whose callee does not share the caller's convention and result, **or passes a value in memory that fits none the caller received**", but `Form::what()` returns only `"a tail call of a function with another convention or result"` (the second cause, produced by `reuse_received`'s `Unlowered::unsupported(self.at, Form::Tail)` at `lower/func/tail.rs:43`, prints a diagnostic that names the wrong reason).
  Why: a program refused because a memory argument fits none of the received regions is told it failed "with another convention or result", which is false and sends the author down the wrong path.
  Fix: make `what()` say both, e.g. "a tail call of a function with another convention or result, or passing values in memory that fit none it received".

- [P2] The verifier forbids a variadic callee for a tail call, but the lowering never checks it - compiler/low/lir/src/verify/calls.rs:53 vs compiler/low/lower/src/func/tail.rs:20-30
  Evidence: `replaces` requires `&& signature.variadic.is_none()`, while `FnLower::tail` only checks `own.conv == signature.conv && own.convention == signature.convention && own.result == signature.result` before emitting `InstKind::Call { kind: CallKind::Tail }`. `invoke` builds `Signature { variadic: Some(..) }` for a C pack callee (`lower/src/signature.rs`), so a `lang.flow.tail` to a C variadic function (`lib/lang/lang/flow.vun` types `tail` from `f`'s signature) would lower successfully and then fail the verifier (`Reason::Tail`) instead of being refused as `Form::Tail`.
  Why: the invariant is enforced in exactly one of the two places that must agree; if such a program type-checks, the result is malformed LIR (a compiler bug) rather than a clean lowering refusal. I could not construct the program within this review, so reachability is unconfirmed.
  Fix: add `signature.variadic.is_none()` to `tail()`'s `shared` condition (or document that the verifier is the sole authority).

- [P2] Memory-argument reuse is a one-way first-fit that refuses feasible tail calls - compiler/low/lower/src/func/tail.rs:36-46 (same shape in compiler/back/wasm/src/build/abi/mod.rs:80-95)
  Evidence: `let mut received = self.b.signature().memory().collect::<Vec<_>>().into_iter(); ... received.find(fits)` - `Iterator::find` consumes up to and including the match, so each callee memory param is matched against the first received region *after the previous match*, never reconsidering earlier ones. The doc says "the first not taken that holds it".
  Why: a function receiving `(M1: 16, M2: 32)` that tail-calls one taking `(32, 16)` matches the 32 against M2, then finds no remaining region for the 16 and returns `Form::Tail`, though M1 fits. A valid program is refused where a valid assignment exists.
  Fix: match greedily but allow backtracking (or sort both sides by size) so a fit is found when one exists; the wasm `reuse` needs the same change.

- [P2] The changeset's own docs disagree about the C backend and record results - docs/spec/decisions.md (652) vs docs/open/language.md:13 and compiler/back/c/src/emit/func/inst.rs:254
  Evidence: decision 652 states "the C backend returns records and results in memory", but `tail_call` still has `Some(AbiType::Record(_) | AbiType::Wide { .. }) => return Err(Missing::TailCall.at(at))`, and `docs/open/language.md` (edited in the same change) lists "a C function's result in memory" as lacking. Decision 650 also says C implements tail calls "for register-only signatures".
  Why: three statements in one changeset cannot all be true; a reader cannot tell whether `#[abi.c] fn -> Record` tail calls are supported.
  Fix: state one truth (C refuses `Record`/`Wide` results) in decision 652 and drop the "records ... in memory" clause.

## Checked, clean

- `lower/func/tail.rs` + `lower/func/call.rs` + `lower/func/mod.rs`: `landing = self.out` for `CallKind::Tail`, `values.extend(landing)` last, `received` captured from the entry params, so `args.last() == out` holds; `invoke` writes each argument into a fresh frame `temp`, so `reuse_received`'s staged copies cannot read a clobbered source (the staging is defensive only).
- `lir/src/verify/calls.rs::replaces` + `verify/reason.rs` + `verify/types.rs::term`: `own.result == signature.result`, `conv`/`convention` equality, `variadic.is_none()`, and `!through() || args.last() == out` are mutually consistent with `Signature::values()` appending the result address last and `returns()`/`through()` agreeing; the new verifier test exercises both accept and reject.
- `passes/src/legalize/rewrite/{call,term,mod}.rs`: `Parts::Written` is inserted for the tail-call result, consumed only by the `Return` arm (dominance makes any other use impossible, so the `unreachable!()`s in `one`/`limbs`/`flat` are genuinely unreachable), and the legalized signature/`self.result` are derived from `legalize.signature`, matching the appended pointer.
- `low/abi/src/lib.rs::Passing::replaces`: the new definition (no `Loc::Indirect`, `stack <= caller.stack`) matches the frame-reuse argument and the sibling backends (`native/core/src/select/frames.rs:70`, `native/x86_64/src/isel/call.rs:29`) and the wasm `reuse`; the old result check it dropped is now covered by the LIR verifier's address check.
- `lir/src/abi.rs` (moved out of `func.rs`): `values`/`passed`/`memory`/`through`/`returns`/`well_formed` are consistent; `func.rs` re-exports keep `vun_lir::AbiType` etc. working; both files are under the 300-line limit (`func/call.rs` 271, `func/mod.rs` ~267, `legalize/rewrite/mod.rs` ~167, `legalize/mod.rs` ~260, `abi.rs` 175).
- `lir/src/analysis/stack.rs`, `analysis/calls.rs`, `analysis/taken.rs`, `inst/operands.rs`, `verify/structure.rs`, `build.rs`: no defect found in the tail-call-relevant paths (an indirect tail call is still `Unbounded::Indirect`, which is correct since the callee's stack use is unknown).

## Open questions

1. Is a `lang.flow.tail` to a C variadic function actually reachable (does `Params(F)` include a pack)? That decides whether the `variadic` asymmetry is a latent gap or a live compiler bug.
2. Is the order-sensitive memory reuse in `reuse_received`/wasm `reuse` intended as a deliberate "in order" rule (both are documented that way) or an oversight? If intended, the doc's "the first not taken that holds it" should say "in order".
3. Should `deepest` in `StackUse` stop adding `frame` for a tail call to an import (it currently takes `max(import.stack_max, frame + deepest)`, overestimating by one frame)?
4. Is `docs/open/language.md` the authoritative open-work list (so the stale `TailResult` line is a real tracker entry), or is it descriptive prose?