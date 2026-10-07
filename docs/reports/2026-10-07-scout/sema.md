## Semantics: names, types, inference, literals/integer semantics, const eval, asm, lints, checks

I read the sema workspace end to end at the seam level (`int/`, `float/`, `ty/`, `typeck/`, `lint/`, `check/`, `asm/`, `core/`) and verified each finding against the code. Two genuine issues plus one duplication; no P0 found.

- [P1] `#[layout.offset(N)]` with a large `N` overflows `u64` and panics the compiler (or silently wraps in release) instead of being reported - compiler/sema/check/src/layout.rs:127 (and :125)
  Evidence: `let at = match field.offset { Some(at) if at < end || at % layout.align != 0 => {...} Some(at) => at, None => end.next_multiple_of(layout.align) }; end = at + layout.size;` — `field.offset` comes straight from user input via `env.count` (`u64::try_from(n.to_i128()?).ok()`, core/src/env/attr.rs:87-90), so it can be `u64::MAX`; for the first field `end == 0`, so `at < end` is false and `at % 1 == 0` passes, then `end = at + layout.size` overflows. `Layout::placed` (layout/shape.rs:96) uses `checked_add`/`checked_next_multiple_of` for exactly this, so the check crate is the outlier.
  Why: `struct S { #[layout.offset(18446744073709551615)] x: u8 }` panics a debug compiler on `attempt to add with overflow` (or, in a release build, wraps `end` to `0`, so the next field's placement is validated against a bogus end and an invalid layout is accepted). The struct's own `Shapes::layout` returns `None`, but `Checker::layout` still runs `offsets` over each field's own layout, so the path is reached.
  Fix: use checked arithmetic here as `Layout::placed` does (`end = at.checked_add(layout.size)?` with `None` mapped to a reported `errors::LAYOUT_OFFSET`), and `checked_next_multiple_of` for the `None` arm. Better, have `offsets` reuse `Layout::placed` on the fields instead of re-deriving placement, so the two placements cannot drift.

- [P2] `Value::literal` silently drops the whole literal when a compile-time integer is past `i128`, though spec 04 makes compile-time integers unbounded - compiler/sema/ty/src/literal.rs:26
  Evidence: `Self::Int(n) => Literal::Int(n.to_i128()?),` — the `?` turns an out-of-range integer into `None` for the whole value, and `Literal::Int` is `i128` (front/syntax/src/literal/mod.rs:44). Every enclosing `Record`/`List`/`Call` then also returns `None` (`all(...)` propagates with `?`).
  Why: a package plan (or any value the literal writer prints) holding a computed integer such as `1 << 200` prints nothing at all rather than that integer, with no diagnostic — a silent failure at the edge the doc says the writer serves.
  Fix: widen the literal integer (`Literal::Int` to the arbitrary-precision `Integer`), or have `literal()` return a typed `Unwritable` reason that the caller reports, instead of `Option::None` that erases the failure.

- [P2] `Integer::bits` and `Integer::magnitude_bits` are two names for one computation - compiler/sema/int/src/lib.rs:77 and :118
  Evidence: `pub fn bits(&self) -> u64 { self.0.magnitude().bits() }` and `pub fn magnitude_bits(&self) -> u64 { self.0.magnitude().bits() }` — identical bodies; `bits()` is used in float/round.rs, float/parse.rs and int/interval.rs while `magnitude_bits()` is used in int/bits.rs and int/interval.rs for the same fact.
  Why: "a symbol means one thing everywhere" and "declare a fact once"; two names invite a caller to assume they differ, and a future change to one silently diverges from the other.
  Fix: keep one (the doc of `magnitude_bits` is the clearer description) and delete the other, updating the call sites.

## Checked, clean

- `int/`: interval arithmetic (`add`/`sub`/`mul`/`div`/`rem`/`magnitude`/`hull`/`meet`), `Interval::new`/`closed`/`half_open`/`items` never build an empty interval; `div`/`rem` stay monotone and sign-correct (verified the `[max(min(lo,0),1-m), min(max(hi,0),m-1)]` bound and the unbounded-divisor case); `bits()` two's-complement width and `is_unsigned_width`/`unsigned_width`; `Integer::shifted` sign/amount handling and the `MAX_BITS` cap; `bitwise`/`complement`; `Extended::add`/`neg`/`mul`/`div` and the `product_limit` zero-times-infinity rule.
- `float/`: `round` (ties-to-even, subnormal field encoding, `q == high` renorm, overflow to infinity), `parse`/`beyond` (the `2^3 < 10 < 2^4` exponent bounds keep `pow` bounded), `ops` NaN/infinity/signed-zero and `mul_add`/`rem`/`compare`/`total_order`, `format` field derivations, `truncated` toward zero.
- `ty/`: `accepts` (integer containment, borrow-serving, slice-of-array, code-of-value, `made_with_empty`), `join`, `stored_as`, `outside`/`refuses`, `narrow` (`To`/`Wrap` modulo/`Sat`), `repeat`/`part`/`at`/`replace` bounds checks, `Endpoints::items`/`run`, `Part::position`, `Display` width and `Power`/`Width::of`, `borrow` helpers. `infer` builds only declared shapes; `expect`'s `reinterprets` correctly rejects same-range/different-size reinterpretation.
- `typeck/`: `infer` per form, `operator`/`dispatch` (interval width cap, `never` propagation), `narrow::facts`/`narrowed`/`after`, `pattern` (tuple rest, or-alternatives types, value/range tested as operators per decision 273), `exhaust` usefulness (`segments`/`specialized`/`rebuilt`, unknown-counts-as-covered), `sequence` array/tuple joins, `range` `Interval::items`.
- `lint/`: every `Lint` diagnostic carries the lint's path id and an origin; `Level::of`/`name`/`capped`/`severity`; `Configured::level` priority and "nothing lowers forbid"; `Walk` allow-scope enter/leave; `pinned` reads `Typed.binders` as decision 351 says; `defined`/`tables` handles derived from `vun_items`.
- `check/`: `layout` `bits`/`tag`/`align` (each with an id, origin, note and help), `holes`, `rest`, `placed`, `visible`, `members`, `gated`, `impls`, `c_layout`; `Checker::namespace` absorbs `TypedDecl`/`Linted` per member.
- `asm/`: `check` entry/label/returns flow analysis, `inst` form selection and `misfit` diagnostics, `read` operand forms, `parts` width selection, `stack` frame/`carried`/`scope` with the `depth as usize` bound guarded, `table` `ArchTable` reading and `callables`.
- All diagnostics in these crates are constructed with an `errors::*` `DiagId` and `.at(origin, ..)`; no `unimplemented!`/`todo!` in the sema tree; no `unsafe`; `unwrap_or(u32::MAX)` in float/ops.rs:22 and the `expect`s in int/interval.rs:164 and float/round.rs are unreachable for the bounded exponent range.

## Open questions

1. Is the `Literal::Int(i128)` width intended to cap printable compile-time integers, or is a wider literal format planned? It decides whether the literal.rs:26 finding is a bug or a known edge.
2. `Value::ty()` returns `None` for an empty array (`value.rs:145`) and `Value::items()` returns `None` for an unbounded range: are these deliberate "no type/no items" answers, or should they be reported (decision 472 names `types.no_items`)?
3. `#[layout.offset]` has no upper bound or "fits the struct" check beyond the placement check — should an offset past any representable size be `layout.offset`'s own error, independent of the overflow fix?

## Coverage note

Reviewed `compiler/sema/*` by module as listed. I could not run `cargo xtask lines` or the test suite (no shell tool), so the 300-line limit and lint/corpus status are unverified here; the parent should run `cargo fmt && cargo clippy --all-targets && cargo nextest run && cargo gen --check && cargo xtask lines && cargo xtask layers`. Nothing in the reviewed files exceeded the limit in the read window.