# Scouting the compiler (2026-10-07)

A health sweep of the whole workspace: eleven fresh-context, read-only review lanes plus one
performance lane, run over the working copy `qtxynwvs` while the in-flight guaranteed-tail-call
change was still being edited. Each lane had the project's own code rules as its standard, had to
quote the code that proves a finding, and reported `path:line`, evidence, consequence and a fix.
The per-lane reports are the files beside this one; this file indexes them, removes duplicates and
ranks what to fix.

Verdict: the tree is in good shape. The verifiers, origins, query engine, encoders, std algorithms
and the new tail-call machinery came out clean wherever a lane read them closely, and no P0 in the
sense of wrong output on an ordinary program was found. What the sweep did find is six live P1s and
one measured cost P0, most of the P1s being one of two shapes: a *check that disagrees with the
thing it guards* (x86-64 asm fit, the tail-call memory reuse search) or *an error that is flattened
or misnamed* (disassembly, layout offsets, the tail-call refusal message). The tail-call change
itself carries the usual merge litter. The single largest cost in the compiler is the cold memo
save, which writes 79.5 MB and takes 1.8 s of a 5.8 s `vun check lib/std`; the parent reproduced
that measurement.

## Triage (2026-10-08)

Every lane was re-read against the tree by a second pass, and the priority list was acted on. The
report overstates in places; what holds and what was done:

- **Fixed:** the memory-argument reuse search (list 2, lowering and wasm, with a driver fixture and
  a wasm test); `Remap` and asm outputs (1, `Rvalue::outputs_mut`); `layout.offset` overflow (3,
  `Misplaced`); `vun-disasm` errors (6); `Volatile` under Cranelift, which its alias analysis
  merged and dropped (backends lane, now atomic accesses); `Form::Tail` split from `TailMemory`
  and the variadic check; the dead `Taken`; `Unlowered::source`; the witness fanout (perf P1,
  `PackageImpls`); `StackUse` (cached per-function contributions, a set); `Move::sequence`;
  decision 652's wording; the `unsafe` exception list; the inline test module; the saved-deps
  decode failure is logged; an LSP offset inside a character; a zero `lang.cover.size`.
- **Wrong:** list 4. x86-64 memory the assembler cannot encode (an index register, an absolute
  address) is accepted by `fit` on purpose (`corpus/accept/capabilities/asm.vun` uses
  `mem(xs, scaled(i, 8), 16)`); the backend reports `backend.lacks` where it is built. Perf P0
  names a double encoding that does not happen (the shared file is 0.4 MB for `lib/std`); the
  59 MB is dependency edges, mostly `typed`, `metered_call` and `mir`, and the witness fanout was
  4% of the file.
- **Overstated:** list 7 (the HIR strings) is not a P1, nothing misbehaves silently; the generator
  identifier panic is only `raw` of `self`, `Self`, `super`, `crate` or `_`.
- **Stale by the time of the triage:** list 5 and the merge items (`lines` passes; the `tmp_*`
  fixtures and the `TailResult` line are gone).
- **Left,** in `docs/open/`: the dead epilogue after a tail jump, the three `put` encoders and
  `Cond` maps, arm64 `apart`, the silent limits (tuple index, `i128` literals, `u16` declaration
  number), the `derive` prelude question, and four performance items.

## Caveat on the baseline

The working copy moved during the sweep (the parent commit changed twice, Cranelift gained
`compile/{mod,function,lowering}.rs`, `entered.rs`, `func/stub.rs`, `tail.rs`, and the `TailResult`
line in `docs/open/language.md` was rewritten). Line numbers in the per-lane files are from the
state each lane read. Findings that were fixed under the sweep are listed under "Resolved during
the sweep"; everything in the priority list was re-read by the parent at the end.

## The priority list

Live P1s first, then the tail-call merge items, then the themes.

1. **`Remap` does not renumber an inline-asm block's output places, so expanding a `Code(T)` callee
   silently writes an unrelated caller local.** `compiler/mid/mir/src/remap.rs:31` remaps operands
   plus the places in `Ref`/`IsVariant`/`Resume`/`End`, and `Rvalue::operands_mut` for `Asm` yields
   reads only (`compiler/mid/mir/src/rvalue.rs:150`). `expand` pushes the callee's locals after the
   caller's and remaps each statement (`compiler/mid/mir/src/build/expand.rs:89`), so an asm output
   keeps the callee's local index, which is always in range and therefore accepted by the verifier.
   The code that fixes it already exists: `Rvalue::outputs()` (`rvalue.rs:191`) returns the asm
   outputs. Fix: remap `asm.bindings_mut()` apart from the reads, the idiom
   `low/passes/src/outline/mod.rs:164` uses. (lane `mid`)

2. **The memory-argument reuse search consumes the slots it skips, so valid tail calls are
   refused.** `compiler/low/lower/src/func/tail.rs:37-42` does `received.find(fits)` over an
   `IntoIter`; `Iterator::find` drops everything it passes, and the same shape is in
   `compiler/back/wasm/src/build/abi/mod.rs:82-94`. The doc right above it, and
   `docs/spec/09-performance.md:61-63`, promise "the first not taken that holds it". A caller
   receiving `(16, 32)` that tail-calls a callee taking `(32, 16)` matches 32 against the second
   slot, then exhausts the iterator and refuses the 16, though the first slot fits. Fix: keep the
   free slots in a `Vec` and remove only the chosen one, and add the missing wasm unit test for
   exactly that order. (lanes `low`, `diff`)

3. **`#[layout.offset(N)]` with a large `N` panics in debug and wraps in release, and the wrapped
   `end` can let an invalid layout through.** `compiler/sema/check/src/layout.rs:127` does
   `end = at + layout.size` on a `u64` that came straight from user input, where `Layout::placed`
   (`layout/shape.rs:96`) uses `checked_add`/`checked_next_multiple_of` for the same computation.
   `struct S { #[layout.offset(18446744073709551615)] x: u8 }` reaches it. Fix: checked arithmetic
   mapped to the existing `layout.offset` diagnostic, or reuse `Layout::placed` so the two
   placements cannot drift. (lane `sema`)

4. **x86-64's operand fit accepts memory the x86-64 assembler cannot encode, so `vun check` passes
   what `vun build` rejects.** `compiler/isa/x86_64/src/fit.rs:74-76` maps every no-writeback
   memory to the canonical `Mem { base: Rax, disp: 0 }` and never consults `addressable`, while
   arm64 and thumb call theirs inside `Kind::fits` (`isa/arm64/src/fit.rs:110-112`,
   `isa/thumb/src/fit.rs:107-109`). `[rbx + rcx*4]` and `[0x1000]` fit, then fail later from
   `A::addressing` in a backend. Fix: check `X86_64::addressable` in `Kind::fits` before
   substituting the canonical operand. (lane `isa`)

5. **A check that currently fails: `cargo xtask lines` reports
   `compiler/back/cranelift/src/tests.rs` at 319 lines**, over the 300 limit, and
   `docs/HANDOFF.md` still says the checks are clean. The parent re-ran this. Fix: split the file
   (the tail-call tests added by the in-flight change are a natural split), and correct HANDOFF.
   (lanes `rules`, `diff`; verified by the parent)

6. **`vun debug disasm` flattens foreign errors into `Malformed(String)` and names no `source()`**,
   breaking two rules at once: `compiler/tools/disasm/src/code.rs:58-76` holds
   `Malformed(String)` with an empty `Error` impl, and `code.rs:94,97` and `elf.rs:28` fill it with
   `e.to_string()` from `vun_elf`/`vun_pe`. Fix: a typed cause plus `source()`; Display stays for
   rendering only. (lane `rules`)

7. **HIR lowering writes `lang`, `impl` and `type` as string literals and builds the path from
   them**, bypassing the generated handles that already exist
   (`compiler/front/hir/src/lower/implementation.rs:22-25`, used at `:90`, `:108`, `:113-116`,
   against `vun_items::lang::{r#impl, r#type}`). Today's behavior is right, so this is drift
   waiting to happen: renaming `lang.type` breaks witness lowering with nothing tying the two
   together. Fix: name both in `lib/lang/lang/syntax/desugar.vun` as `Item` paths, as `markers`
   already does, and read the generated table. (lane `front`)

## Performance (lane `perf`)

Measured on `target/release/vun check lib/std` with a fresh store. See [`perf.md`](perf.md) for the
full evidence and the five smaller items.

1. **[P0] The cold memo save writes 79.5 MB and costs 1.8 s of a 5.8 s check (31%).** The project
   file (61.8 MB) is written whole, so the library memos that `share()` just wrote to the shared
   file (17.7 MB) are encoded a second time, and values already encoded in memory are re-encoded
   (`base/query/src/persist/query.rs:150`, `mod.rs:208,234`, `tools/driver/src/queries.rs:120,176`).
   Fix: attach the shared file as a base (`Keeps::Base`, which `Writer::over` already supports)
   before saving the project body, and keep the encoded bytes beside the value.
2. **[P1] The witness lookup reads every file's `Impls` and `Derives` for every `(file, type)`:**
   ~3882 computes each scanning ~265 file memos, that is ~1M `cx.get` calls and 709 ms own, and
   every read is persisted as a dependency, which also inflates the store the P0 above writes
   (`sema/core/src/witness/lookup.rs:61,64` with `witness/mod.rs:260`). Fix: one per-package
   `Impls` aggregate, or an index from an impl's target head to its impls.
3. **[P1] `StackUse::deepest` is O(k^3) over a chain of k tail-calling functions:** the tail chain
   is walked once per member and each link is found with a linear `reached.contains`
   (`low/lir/src/analysis/stack.rs:106,119`). Reasoned, not measured; the parent confirmed the
   shape by reading the code. Fix: memoize the decomposable per-function chain value and use a set.
4. Five P2s: `Concrete::propagate` advances one constant level per full-body scan;
   `Move::sequence` orders k parallel copies in O(k^3); `Table::entries()` re-hashes every key on
   each save and load; the lint pass rebuilds each node's descriptor once per visible lint; the
   layered save scans `keys.replaced` linearly per key.

## Before merging the tail-call change

These are small and all in the working copy, from the `diff` lane's read plus the parent's checks.

- `compiler/low/lir/src/analysis/taken.rs` is dead: `Taken`, its `pub use`, and the module doc
  describe an "address taken" rule nothing computes or reads any more. Verified: no consumer
  outside its own definition.
- `compiler/tools/driver/tests/driver/programs/tmp_{chain,plain,tail,widen}.vun` are unreferenced
  scratch fixtures, two of them duplicates of `tail_aggregate.vun`. Verified present. Delete.
- Native and wasm emit `returned` plus `epilogue` after the unconditional tail jump, where Cranelift
  and C skip them (`native/core/src/select/term.rs:87-90`, `back/wasm/src/build/control.rs:175`,
  `func.rs:196` versus `cranelift/src/func/term.rs:43-44`, `c/src/emit/func/term.rs:41`). Dead code,
  but it doubles a CFI `Returned` step. Fix in the framework so every backend skips it.
- The new tests do not test what they claim: `tails_pass_values_in_memory` asserts only that the
  call text is gone, the wasm test checks the reuse index but never the copy order, and the
  legalize test asserts only `unit.verify == Ok(())` (`native/core/src/testing.rs:127-144`,
  `back/wasm/src/build/abi/tests.rs:15-33`, `low/passes/src/legalize/tests.rs:223-248`).
- `Form::Tail`'s message still says "another convention or result" for the case where only no
  received memory fits, and the lowering never checks the `variadic.is_none()` rule the verifier
  enforces (`low/lower/src/func/tail.rs:20-30` against `lir/src/verify/calls.rs:53`);
  reachability of the second was not proven.
- Decision 652 says "the C backend returns records and results in memory", but `tail_call` refuses
  `AbiType::Record` (`back/c/src/emit/func/inst.rs:252-254`) and the test filters the C backend out
  (`driver/tests/driver/backend/forms.rs:44-53`). One of the three statements should change.

## Coverage

| Lane | Area | Findings | File |
|------|------|----------|------|
| `front` | lexer, parser engine, CST, HIR, desugar, grammar and primitive tables | 1 P1, 3 P2 | [`front.md`](front.md) |
| `sema` | names, types, inference, integer/float, const eval, asm forms, lints, checks | 1 P1, 2 P2 | [`sema.md`](sema.md) |
| `mid` | MIR and its passes | 1 P1, 2 P2 | [`mid.md`](mid.md) |
| `low` | LIR, lowering, legalization, layout, ABI | 4 P2 | [`low.md`](low.md) |
| `backends` | C, wasm, Cranelift, regalloc, codegen, native ISel, asm core | 3 P2 | [`backends.md`](backends.md) |
| `isa` | target instruction descriptions and encodings | 1 P1, 2 P2 | [`isa.md`](isa.md) |
| `infra` | base (query, diag, data, codec, uri, par, fs, term, trace, hash, span, component, html), object, link | 3 P2 | [`infra.md`](infra.md) |
| `tools` | CLI, driver, xtask and generators, LSP, command, doc, disasm, harness, components | 3 P2 | [`tools.md`](tools.md) |
| `pkg-lang` | packages, `lib` lang and std, corpus, generated handles | 2 P2 | [`pkg-lang.md`](pkg-lang.md) |
| `rules` | whole-repo convention inventory (the code rules) | 1 P1, 8 P2 | [`rules.md`](rules.md) |
| `diff` | the in-flight guaranteed-tail-call change | 1 P1, 6 P2 | [`diff.md`](diff.md) |
| `perf` | cold save, witness fanout, stack walk, allocations | 1 P0, 2 P1, 5 P2 | [`perf.md`](perf.md) |

## Cross-lane themes

- **A rule enforced in one of the two places that must agree.** The x86-64 fit (P1 4), the
  tail-call verifier against the lowering (variadic), the layout placement checked in `check`
  against `Layout::placed`, and `assign` versus `compound` in the grammar tables. Each wants either
  one implementation or a test that the two agree.
- **Silent degradation instead of a diagnostic.** A tuple index past `u32` becomes `Name::Hole`
  (`front/hir/src/lower/mod.rs:184-186`), a compile-time integer past `i128` erases the whole
  literal (`sema/ty/src/literal.rs:26`), a mid-codepoint LSP offset becomes column 0
  (`tools/lsp/src/position.rs:51`), and a corrupt saved-deps blob is indistinguishable from a
  missing dependency (`base/query/src/memo.rs`, `Deps::get`). The project's rule is the opposite:
  a typed reason at the edge.
- **Two names or two lists for one fact.** `Integer::bits` and `magnitude_bits`
  (`sema/int/src/lib.rs:77,118`), `Arch::ALL` plus the `namespace`/`shape` matches
  (`sema/asm/src/lib.rs:39`), the three copies of `Cond` to condition code, the three copies of the
  encoding backends' `put`, and the hand-written prelude list. All are the "declare a fact once"
  rule.
- **Whole classes of drift left unshared.** The three `put<S>` encoders have already drifted in
  signature (arm64/thumb take the word, x86-64 encodes it), and x86-64's `copy_bytes` uses
  `expect("fits")` where the shared `Carries::copy_bytes` steps far offsets.
- **Documentation that is now wrong.** The `unsafe` exception list is stated three different ways
  (CLAUDE.md names two, `vun-component` says "besides `vun-par`", `vun-clang` says "the only
  crate"), HANDOFF says the checks are clean, `Registered::uses` carries `operand`'s doc, and the
  lexer's `offset` carries a removed string helper's doc.

## Resolved during the sweep

- `docs/open/language.md` named `Unlegalized::TailResult`, a variant the same change deleted, and
  reported the now-implemented wide tail result as missing. The file was rewritten while the lanes
  ran; `rg TailResult` is now empty there. (`low`, `diff` both reported it)

## Open questions for the owner

1. Is `vun-clang` a deliberate third `unsafe` exception (decision 131)? If yes, name all three in
   CLAUDE.md and write its soundness argument beside the `allow`, as the other two do.
2. Is the order-sensitive memory reuse in `reuse_received`/wasm `reuse` intentional ("in order"),
   or a slip against the spec's "first not taken that holds it"? This decides bug fix versus spec
   edit, and it is the one P1 that changes program behavior.
3. Does the C backend intend to support a `#[abi.c]` record result at all? Decision 652, the code
   and the test currently give three answers.
4. Should `#[derive(f)]` resolve bare, as the `lang.vun` docs and `impl` suggest, or should the
   doc say `#[lang.derive(f)]`? The prelude exports `impl` but not `derive`.
5. Is `Literal::Int(i128)` meant to cap printable compile-time integers, and is `Anchor::Declaration
   { nth: u16 }` a deliberate declaration-count budget? Both are silent limits today.
6. `cargo xtask check` runs `layers`, which writes `assets/diagrams/layers.toml`. Should a check
   command mutate the tree?

## Verified by the parent

Re-read after the lanes returned: the `Remap`/`Rvalue::outputs` shape (P1 1), `reuse_received` and
its doc (P1 2), `layout.rs:127` (P1 3), `isa/x86_64/src/fit.rs:74-76` (P1 4), the `lines` failure
and the `tmp_*.vun` fixtures and the unused `Taken` (P1 5 and the merge items), `vun_disasm::Error`
(P1 6), `docs/open/language.md` (resolved), and the performance P0, whose numbers were reproduced
against a fresh throwaway store (1841 ms save, 17 688 422 and 61 766 917 byte files). The witness
fanout and the `StackUse` walk shapes were also read in the parent. The remaining findings are as
their lane reported them and were not independently re-read.
