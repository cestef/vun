# 09. Performance

Performance is part of the semantics: what the language guarantees determines what every backend
may assume. Compile-time performance of the implementation is in spec 13.

## 1. Guarantees the optimizer may rely on

| Fact | Source | Used for |
|------|--------|----------|
| exclusive borrows do not alias | spec 05, section 8 | `noalias` / `restrict`, load and store reordering |
| `&T` of cell-free `T` is immutable while borrowed | spec 05, section 8 | read-only, hoisting loads out of loops |
| borrow parameters do not escape into free results | spec 05, section 4 | `captures(none)`, stack promotion |
| integer ranges | spec 04 | check elimination, narrower registers, niches |
| no unwinding | spec 07 | no landing pads, smaller code |
| function addresses are not unique | spec 05, section 7 | merging identical instantiations |
| struct layout is fixed and known | spec 05, section 6 | no hidden reordering, predictable caches |
| frame layout is specified | spec 06, section 5 | coroutine sizes known at build time |

## 2. Checks

Bounds checks are the dominant cost of safety in comparable languages (research notes, section 3:
about half of Rust's checking overhead). Vun's approach:

- Overflow checks do not exist: arithmetic is exact, narrowing is explicit (spec 04).
- Indexing with a range-typed index of the right bound is unchecked by construction; `for i in 0..N`
  over a compile-time `N` produces such indices.
- The remaining checks are removed by MIR range analysis when provable. The rest stay, identically
  in every build profile (guarantee 3).
- All trap sites branch to one shared cold stub per function with a site id; messages live in a
  non-loaded table (spec 07). Checks cost a compare and a branch, not a formatted message.

## 3. Controls

All performance controls are `lang` attributes or functions. None is a keyword and none changes
semantics.

| Control | Spelling | Notes |
|---------|----------|-------|
| cold code | `#[opt.cold]` on a function; `lang.hint.cold()` in a branch | branches ending in a `never` call are cold by rule, so trap paths are cold |
| inlining | `#[opt.inline(.always)]`, `#[opt.inline(.never)]` | `.always` is an error if the call cannot be inlined; inlining never changes meaning |
| optimize for size | `#[opt.level(.size)]` on a declaration | per declaration, overrides the build profile |
| tail calls | `return lang.flow.tail(f, (a, b))` | guaranteed; see below |
| prefetch | `lang.mem.prefetch(p, .read, .l1)` | safe; a hint only |
| vectors | `lang.simd.Vector(N, T)` | spec 04, section 3 |
| target features | `#[target.require(.{ .avx2 })]` | spec 04, section 3 |
| float reassociation | explicit functions only | spec 04, section 2 |
| layout | `layout.*` attributes, `collections.Soa(T)` | spec 05, section 6 |

There are no `likely`/`unlikely` hints: `cold` covers the cases that matter, and it is the one hint
both Zig and Rust converged on.

### Guaranteed tail calls

`return lang.flow.tail(f, args)` jumps to `f` without growing the stack.

- It must be in return position; the callee's ABI and result type must match; no argument may be a
  borrow of the caller's locals.
- Pending `defer`s of the caller run before the jump.
- A backend without tail-call support rejects the program; there is never a silent fallback to a
  normal call.
- A cycle of guaranteed tail calls satisfies `stack.max` without `stack.depth`.

This is what threaded interpreters, state machines and parsers need, and it keeps their stack
bounded by construction.

## 4. Generic code size

Monomorphization is the default and the only semantic model. Code size is controlled by:

- merging instantiations whose lowered LIR is identical (`List(u32)` and `List(i32)`), which is
  exact because LIR is type-erased (spec 13);
- `#[opt.level(.size)]` and `#[opt.inline(.never)]` on hot generic helpers;
- `std.Dyn(I)` when dynamic dispatch is the better trade;
- `vun check --meta-stats`, which reports generated code size per generic and per generator.

## 5. Build profiles

Backend, optimization level, LTO sharding and PGO data are build settings (spec 12). Every build
is whole-program at the LIR level, because packages are source-only; code generation stays
parallel by sharding functions by hash.
