#import "../template.typ": *

= Performance model <sec-performance>

Performance is treated as part of the semantics: what the language guarantees determines what an
implementation may assume. @tab-facts lists the guarantees relevant to code generation and their
use. None of them is an optimizer heuristic. Each follows from a rule stated in an earlier section.

#figure(
  table(
    columns: (1fr, auto, 1fr),
    toprule,
    [Fact], [Source], [Use in code generation],
    midrule,
    [exclusive borrows do not alias], [@sec-memory], [`noalias` and `restrict`; reordering of loads and stores],
    [`&T` of cell-free `T` is immutable while borrowed], [@sec-memory], [read-only memory; hoisting loads out of loops],
    [borrowed parameters do not escape into free results], [@sec-memory], [`captures(none)`; stack promotion],
    [static integer ranges], [@sec-integers], [check elimination; narrower registers; niches],
    [no unwinding], [@sec-capabilities], [no landing pads; smaller code],
    [function addresses are not unique], [@sec-memory], [merging identical instantiations],
    [structure layout is fixed and known], [@sec-memory], [no hidden reordering],
    [coroutine frame layout is specified], [@sec-suspension], [frame sizes known at build time],
    bottomrule,
  ),
  caption: [Language guarantees that code generation may rely on.],
) <tab-facts>

== Checks

Run-time checks are the main cost of safety in comparable languages. One study of Rust measured
programs with all checks enabled at 1.96 times the run time of equivalent C, and attributed about
53% of that overhead to bounds checking @rust-checks. In Vun, overflow checks do not exist.
Indexing with an index of the right range type is unchecked by construction, and `for i in 0..N`
over a compile-time `N` produces such indices. Range analysis on the mid-level representation
removes further checks when it can prove them. The checks that remain stay identical in every
build profile (guarantee 3). They branch to one cold stub per function carrying a site identifier,
and the message text lives in a table that is not loaded on embedded targets, in the manner of
deferred logging @defmt. A check costs a comparison and a branch.

== Controls

Performance controls are `lang` attributes and functions, and none of them changes meaning
(@tab-controls). There are no `likely` or `unlikely` hints: a cold marker covers the cases that
matter, and branches that end in a call of result type `never`, which includes every trap path,
are cold by rule.

#figure(
  table(
    columns: (auto, 1fr),
    toprule,
    [Control], [Spelling and behavior],
    midrule,
    [cold code], [`#[opt.cold]` on a function; `lang.hint.cold()` in a branch],
    [inlining], [`#[opt.inline(.always)]` is an error when the call cannot be inlined; `.never`],
    [size], [`#[opt.level(.size)]` per declaration, overriding the build profile],
    [tail calls], [`return lang.flow.tail(F, f, (a, b))`, specified as guaranteed or rejected],
    [prefetch], [planned: `lang.mem.prefetch(p, .read, .l1)`, safe, a hint only],
    [vectors], [`lang.simd.Vector(N, T)` with scalar range rules],
    [floating point], [explicit fused and reassociating operations with a specified reduction order],
    [layout], [`layout` attributes; struct-of-arrays as a library type],
    bottomrule,
  ),
  caption: [Performance controls. Each is a `lang` item; none is a keyword.],
) <tab-controls>

A guaranteed tail call must be in return position. The callee's calling convention and result type
must match, and no argument may borrow the caller's locals. Pending deferred statements of the
caller run before the jump. A backend that cannot honor the guarantee rejects the program; the call
is never silently compiled as an ordinary call. A cycle of guaranteed tail calls satisfies a stack
bound without a declared depth, which is what threaded interpreters, state machines and parsers
need.

== Generic code size

Monomorphization is the only semantic model of generic code. Code size is controlled by merging
instantiations whose lowered, type-erased code is identical (`List(u32)` and `List(i32)`, for
example), which is exact because the low-level representation carries no source types; by size
optimization and inlining controls on generic helpers; by dynamic dispatch (the planned `std.Dyn(I)`) where it is
the better trade; and by `vun check --meta-stats`, which reports the instantiations and generated code size of each generic. Packages are
distributed as source, so every build sees the whole program at the low level. Code generation
remains parallel by sharding functions by hash.
