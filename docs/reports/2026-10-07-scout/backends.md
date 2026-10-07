## Code generators and native instruction selection

I read the framework (`back/codegen`), the regalloc checker, the wasm build/verify tables, the C emit paths, the three native ISel selectors, the shared `native/core` selection/carry, and the asm-core buffer. Everything below is what I could substantiate from the code read; I did not find a P0/P1 within the budget and I did not run the corpus or the `compare` harness (read-only), so this is a reading review.

- [P2] Cross-backend drift: `Access::Volatile` is honored only by the C backend; native and wasm silently drop it - `compiler/back/c/src/emit/func/inst.rs:35` and `:47` (emits `Ty::Volatile`) vs `compiler/native/arm64/src/isel/select.rs:130`, `compiler/native/thumb/src/isel/select.rs:176`, `compiler/back/cranelift/src/func/inst.rs:138` (`Access::Plain | Access::Volatile =>` same load), `compiler/back/wasm/src/build/inst.rs:42` (`access: _`)
  Evidence: C branches `Access::Volatile => self.operand(addr).at(Ty::Volatile(...))`; native branches only on `access.ordered()` (false for `Volatile`) and emits the plain `LOAD`/`STORE`; wasm matches `Load { .., access: _ }`.
  Why: a program calling `lang.mem.volatile_load`/`volatile_store` (lowered to `Access::Volatile` at `low/lower/src/func/memory.rs:96`) compiles with `volatile` in C output but a plain access from native/wasm/cranelift, with no `Lack` reported (`Missing` has no `Volatile` variant). For an mmio target that is a silent behavioral difference between backends.
  Fix: either add a `Missing` variant / a `Feature` and report the lack where volatile cannot be expressed, or document that volatile is C-only and reachable only there.

- [P2] The byte-piece copy/load/store shape is written twice and the copies differ on far offsets - `compiler/native/core/src/select/carry.rs:96` (`Carries::copy_bytes`, used by arm64/thumb) vs `compiler/native/x86_64/src/isel/carry.rs:88` (`Sel::copy_bytes`)
  Evidence: the shared one moves both addresses up when a piece is out of reach (`Self::offset(cx, from, from, at - moved)`, then restores `-moved`), while x86 uses `from.plus(at).expect("fits")` / `to.plus(at).expect("fits")` per piece with `.expect("records fit 2 GiB")` on the byte loads/stores.
  Why: a legal copy of more than 2 GiB (or one whose pieces exceed the displacement range) compiles on arm64/thumb but panics in the x86-64 selector, and the two algorithms can drift as one is fixed. The rule "extract the shared shape" is not applied here.
  Fix: implement `Carries` for x86 or lift the far-offset stepping into the shared helper; at minimum drop the panicking `expect("fits")` in favor of the reach-and-offset loop.

- [P2] LIR `Cond` to machine condition code is written three times with the same subtle float decisions - `compiler/native/x86_64/src/lib.rs:167` (`condition`), `compiler/native/arm64/src/lib.rs:187` (`condition`), `compiler/native/thumb/src/lib.rs:216` (`Flags::holds`)
  Evidence: arm64 `condition` and thumb `holds` are the same mapping, both carrying `Cond::ULe | Cond::FLe => Cc::Ls`, `Cond::FLt => Cc::Mi`, `Cond::FNe => return None`, and `Cond::Eq | Cond::FEq => Cc::Eq`; x86 has a third copy returning `None` for every float condition.
  Why: these encode the ordered-comparison rule (`FLe` must be `Ls`/`CC`, not `LE`/`GT`, so NaN compares false). A future change or a new `Cond` variant must be applied in three places or a backend silently returns a wrong result for one comparison; the rule lives in data nowhere.
  Fix: one shared mapping in `native/core` (e.g. `flags(cond) -> Option<Cc>`), with each architecture's `Cc` type adapting to it, plus a test that every `Cond` is covered.

## Checked, clean

- Float compare semantics, verified in the snippets and mappings: x86 `FEQUAL` clears parity (`Set NP; And`, `compiler/native/x86_64/src/snippets.rs:153-160`), so `FEq`/`FNe` are ordered; `FCOMPARE` uses `Ucomis` + `Set Cond` with `FLt`/`FLe` ordered by swapping operands (`x86_64/src/isel/select.rs:184-193`). arm64/thumb map `FLt => Mi`, `FLe => Ls`, `FNe => None` (two-condition `FNE` snippet), which is correct under the documented ordered semantics.
- wasm `compare-exchange` operand order (`back/wasm/src/build/inst.rs:187-201`): traced against `verify/mod.rs:211` `select()` (pops cond, then second, then first) and `Store { addr, value }` — stack is `[addr, select(old==expected, new, old)]`; correct.
- wasm selector tables `BINARY`/`COMPARE`/`CAST` (`build/select.rs`) match the LIR enums; `FNe` is intentionally absent from `COMPARE` and handled specially with a comment, so the `.expect("verified LIR ...")` is not reachable from verified LIR.
- wasm `verify` (`module/verify/{mod,control}.rs`) checks underflow, mismatched types, labels, locals, unbalanced arms and leftover operands; `dispatch` guards the i64 index with `I64GeU`+`WrapI64` before `BrTable` (`build/inst.rs:204`).
- regalloc checker (`back/regalloc/src/check.rs`) simulates the plan to a fixed point over the CFG and checks `Placeless`/`Elsewhere`/`NotInReg`/`Reserved`/`Overwritten`; `Pos::read`/`write`/`next`/`before` arithmetic and `Points::of` block/inst indexing are consistent.
- `Buffer::finish`/`refer`/`relocate`/`word`/`rewrite`/`replace` (`asm/core/src/buffer.rs`) patch labels and assert rewrites link nothing; x86-64 `modrm` handles the `rmb`/`rsp` SIB, disp8/disp32 choice, RIP-relative `-4 - trailing` addend and absolute `0x25` forms correctly (`asm/x86_64/src/encode.rs:164-205`).
- No `unsafe` anywhere in `back/`, `native/`, `asm/` (grep returned none), so no unsafe outside the `vun-par`/`vun-component` exceptions. No `todo!`/`unimplemented!`/`FIXME`/`TODO` in the area.
- No ad-hoc `println!` debug output in the area; tracing is through `vun_trace` (e.g. `back/wasm/src/build/inst.rs:27`). The `format!` uses in `back/c` are the C text edge (identifier rendering), which the rules permit.

## Open questions

1. Is dropping `Access::Volatile` on native/wasm/cranelift intended (volatile being a C-only edge), or should it be a reportable `Lack`?
2. Should x86-64's `Sel::copy_bytes` be folded into `vun_native::select::carry::Carries`, or is the x86 selector deliberately standalone (its `load_bytes`/`store_bytes` also duplicate the shared pair)?
3. Is the three-way `Cond -> Cc` mapping intended to stay per-architecture, or is a shared table in `native/core` wanted (an `isa` table would make it data)?
4. `Feature::DebugInfo` is declared for the native backend (`native/backend/src/lib.rs:99`); I saw CFI frame unwinding (`codegen/src/cx/saves.rs`) but did not verify line tables / full DWARF emission — expected to be present?