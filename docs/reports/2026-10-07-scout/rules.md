## Whole-repo convention sweep (code rules, not per-area logic)

Scope note: this is a sampled inventory, not an exhaustive audit. Every category below was driven by `rg` over `compiler/`, and each reported item was verified against the definition or the sibling. Working copy is dirty (`jj st` shows many `M`/`A` files), so line numbers are the current working tree.

- [P1] Two explicit error rules are broken at once in `vun-disasm`: foreign errors are flattened with `to_string()` and the error chain names no `source()` - compiler/tools/disasm/src/code.rs:58-76 (type), compiler/tools/disasm/src/elf.rs:28, compiler/tools/disasm/src/code.rs:94,97
  Evidence: `pub enum Error { ... Malformed(String) }` and `impl std::error::Error for Error {}` (code.rs:58,63,76); `vun_elf::File::read(bytes).map_err(|e| Error::Malformed(e.to_string()))?` (code.rs:94), likewise pe (97), symbols (elf.rs:28).
  Why: `vun debug disasm` on a malformed ELF/PE loses the typed `vun_elf`/`vun_pe` reason and the chain ends at `Malformed`; `source()` returns `None` where the rule requires it to name the underlying error. Any future caller that walks `source()` silently gets nothing.
  Fix: give `Malformed` a typed cause (boxed `dyn Error` or the `vun_elf`/`vun_pe` error types) and implement `source()`; keep Display only for rendering.

- [P2] `Unlowered` wraps other errors and reports none as its source, and one of them (`vun_mir::Unready`) does not implement `std::error::Error` at all - compiler/low/lower/src/unlowered.rs:104,131; compiler/mid/mir/src/concrete/unready.rs:16,34
  Evidence: `impl std::error::Error for Unlowered {}` (unlowered.rs:104) with variants `Why::Unready(Unready)` / `Why::Reported(Reason)`; `Unready` has `impl std::fmt::Display` (unready.rs:34) but no `Error` impl; `Why::Unready(unready) => Msg::new().text(unready.to_string())` (unlowered.rs:131).
  Why: the lowering failure chain is flattened to rendered text. `diagnostic()` special-cases some variants, but the fallback path loses the `Unready` structure, contrary to "errors structured ... names its source()".
  Fix: implement `Error` for `Unready` and add `Unlowered::source()`; replace the `.to_string()` fallback with a structured `Msg` built from the `Unready` parts.

- [P2] `unsafe` has a third exception (`vun-clang`) not in the stated list, and three doc comments contradict each other about how many exceptions exist - compiler/tools/clang/src/lib.rs:4-8 (doc + `#![allow(unsafe_code)]`); compiler/base/component/src/lib.rs:12; docs/spec/12-ecosystem.md:396-397; docs/spec/decisions.md:137 (decision 131)
  Evidence: clang lib.rs says "This is the only crate of the toolchain allowed `unsafe`"; component lib.rs says "the one module of the toolchain besides `vun-par`'s pool that uses `unsafe`"; `docs/spec/12-ecosystem.md` repeats the component claim; `CLAUDE.md` names only `vun-par` and `vun-component` as exceptions.
  Why: three "one rule per concept" statements, at most one true. `vun-clang` is sanctioned by decision 131, so the documented rule is stale and the soundness obligation is only the prose "each call into libclang is wrapped once" (no block-level argument like `pool.rs`/component write).
  Fix: make CLAUDE.md and the two doc comments name all three exceptions and point at decision 131; write the clang soundness argument beside the `allow` as the other two do.

- [P2] Duplicated free `put<S>(asm: &mut Asm<S>, ...)` in the three encoding backends, with a signature drift: arm64/thumb take the encoded `word`, x86_64 encodes inside - compiler/asm/arm64/src/encode.rs:59, compiler/asm/thumb/src/encode.rs:58, compiler/asm/x86_64/src/encode.rs:64
  Evidence: arm64:59 `fn put<S: Copy>(asm: &mut Asm<S>, op: Op, form: &Form, word: u32, size: Size, operands: &[Operand<S>])`; thumb:58 identical; x86_64:64 `fn put<S: Copy>(asm: &mut Asm<S>, op: Op, form: &Form, size: Size, operands: &[Operand<S>])`.
  Why: the same "append word, record relocations, list the instruction" shape is written three times and has already diverged. A fix to relocation or listing in one backend does not reach the others. It is also an orphan function: `Asm` is the crate's own type.
  Fix: make it an associated function/method on the arch's `Asm` (or a shared `vun-asm` helper) with one signature; the per-arch encode step stays in the caller.

- [P2] Orphan free function over a type: `narrow(size: Size) -> bool` - compiler/native/arm64/src/isel/mod.rs:150
  Evidence: `fn narrow(size: Size) -> bool { size < Size::B32 }`. `Size` is `vun_isa_api`/`vun_isa::reg::Size` (compiler/isa/arm64/src/reg.rs:71), a foreign type.
  Why: the rule says a function working on a type is a method or a crate-local extension trait; free functions are reserved for main/tests/table handlers/walk defaults/foreign-to-foreign mappings/generic algorithms. This is a plain predicate on `Size` living as a free fn, so every new predicate copies this shape.
  Fix: move to an extension trait (or `Size::is_narrow`) beside the other `Size` predicates.

- [P2] Without due to the 300-line rule, `compiler/back/cranelift/src/tests.rs` is 309 lines and `cargo xtask lines` fails - compiler/back/cranelift/src/tests.rs (whole file); checker compiler/tools/xtask/src/lines.rs:9-16
  Evidence: `cargo xtask lines` → `error[xtask.lines.oversized]: compiler/back/cranelift/src/tests.rs has 309 lines, more than 300` (exit 1). The checker's `CHECKED` includes `compiler` with extension `rs` and does not exempt `tests.rs`, so this is a real failure. `docs/HANDOFF.md` claims "lines ... are clean", which is stale for the working copy.
  Why: rule violation and a check that currently fails on the tree being reviewed.
  Fix: split `tests.rs` into a `tests/` directory module by concern, or move the tail-call tests added by the in-flight change into a dedicated file; update HANDOFF.

- [P2] An inline `mod tests` lives at the bottom of a source file rather than in a `tests.rs` submodule - compiler/sema/core/src/target/read.rs:246
  Evidence: the only match for `^\s*mod tests \{` under `compiler/` outside `tests.rs`/`tests/` is `compiler/sema/core/src/target/read.rs:246`.
  Why: rule says unit tests live in their own `tests.rs` submodule, never at the bottom of the file they test. This is the single remaining inline test module.
  Fix: move the module to `compiler/sema/core/src/target/read/tests.rs`.

- [P2] Parallel matches over the same enum hold the same fact in more than one place, so a new architecture needs edits in several independent sites - compiler/sema/asm/src/lib.rs:39 (and the `namespace`/`shape` matches just below)
  Evidence: `Arch::ALL` is hand-written (`&[Self::X86_64, Self::Arm64, Self::Thumb, Self::Wasm32]`) and the enum is then re-matched in `namespace()` and `shape()`, plus a per-arch module and generated table.
  Why: the single-source-of-truth rule ("a table over an enum is never written out a second time", `vun-variants`' own doc). Here a fourth site is added for every new arch, and `ALL` can silently drift from the matches.
  Fix: derive `Variants` for `Arch` (or resolve each arch's namespace/shape from one table) and have `ALL` come from the derive.

- [P2] Component/doc drift: the component contract is stated in code and spec but the "only unsafe" claim in `vun-clang` is impossible to satisfy together with the other two - compiler/tools/clang/src/lib.rs:4, compiler/base/component/src/lib.rs:12
  (Captured together with the `unsafe` finding above; listed separately only because the owner should pick one canonical wording.)

### Checked, clean

- `TODO`/`FIXME`/`XXX`/`HACK` in `src/`: zero except the doc string `\uXXXX` in `compiler/base/data/src/json/string.rs:52` (not a marker).
- `todo!()` / `unimplemented!()` in `src/`: zero occurrences.
- Ad-hoc printing in non-test `src/`: zero `println!`/`eprintln!`/`dbg!` outside `tests.rs`/`tests/`; output goes through `vun-term`/`vun-trace`.
- Oversized files: `cargo xtask lines` reports exactly one (the cranelift `tests.rs` above); `find` over non-generated `compiler/**.rs` agrees, and no `lib`/`corpus` `.vun` file exceeds 300 lines.
- `unsafe` sites: outside `vun-par/pool.rs` and `vun-component`, the only other sites are `vun-clang` (reported); `vun-backend-llvm` is a 29-line stub with no libLLVM FFI yet. `vun-par` and `vun-component` both carry their soundness argument in the module doc.
- Feature flags: `cfg(feature = ...)` values are arch/component selectors with real defaults, not always-one flags.
- Generated directories: no evidence of hand edits encountered; not re-run (`cargo gen --check` would mutate/regenerate, so left to the owner).
- Duplicated data tables sampled: release component discovery (`xtask/src/release/components.rs`) and the driver registry are driven from `package.metadata.vun-component` / feature lists, not a second hand table.
- `allow`/`expect` lint escapes: nearly all carry `reason = "..."`; the lone exception (`compiler/base/fs/src/process.rs:11`) is a trivial cast and not worth a finding.
- Swallowed `let _ =`/`.ok()` scan found the `ErrorGuaranteed` pattern (diagnostic already recorded, rendered by `Session::emit`), e.g. `compiler/tools/cli/src/commands/new.rs:36`; not an error-loss bug.

### Open questions

1. Is `vun-clang` an intentional third `unsafe` exception (decision 131) that CLAUDE.md should name, or should the libclang adapter be moved under the component boundary?
2. Should `Why::name()`/`GapKind::stage()` text labels (`coverage.rs:184`) be promoted to a typed enum, or is the summary string genuinely at the edge?
3. Is the hand-written `const ALL` pattern (no `vun-variants` dependency in several crates) a deliberate dependency choice, or should those crates adopt `#[derive(Variants)]`?
4. Should `vun_disasm::Error` and `vun_lower::Unlowered` be required to name `source()`, given both currently render structured causes to text?
5. The `Malformed`/`Errors(Vec<String>)` string variants at the component boundary (`vun-component/src/cause.rs`, `binder/bindings.rs`) are forced by the no-Rust-types rule; do they need a typed-on-this-side wrapper before display?

### Top 10 to fix now

1. Fix `vun-disasm` error flattening + missing `source()` (P1).
2. Give `Unlowered` a `source()` and make `vun_mir::Unready` an `Error` (P2).
3. Reconcile the three `unsafe` doc claims and add the clang soundness argument (P2).
4. Split `compiler/back/cranelift/src/tests.rs` to restore `cargo xtask lines` (P2).
5. Move `sema/core/src/target/read.rs` tests into `tests.rs` (P2).
6. Unify the three `put` encoders into one shared shape (P2).
7. Turn `narrow(Size)` into an extension trait/associated fn (P2).
8. Derive `Arch::ALL` or table-ize `namespace`/`shape` (P2).
9. Update `docs/HANDOFF.md` (says lines clean; it is not).
10. Decide the typed labels for coverage gaps (`stage()`/`name()`).