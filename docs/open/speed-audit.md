# Open: speed audit (2026-10-08)

Measured on `am3.wg`, pinned, from the benchmark of `scripts/bench-compare` and `vun debug profile`.
Complements [`performance.md`](performance.md), which holds the per-stage findings.

## Where Vun is slow

| Case | Wall time | Note |
|------|-----------|------|
| Cold build (empty store), `fib` | ~345 ms | 1.2 s of CPU work, threads mostly idle |
| Edit a constant, rebuild | ~100 ms | |
| Unchanged rebuild | ~20 to 36 ms | near gcc (32 to 45 ms) |

Cold build, CPU time (`vun debug profile build-hello --phase cold --measure time`):

- `parsed` 21% (253 ms, 316 library files; ~7000 instructions a token, 11 MiB/s).
- Waiting 25%: claims 15%, workers 10%. A thread blocked on an in-flight query does no other work.
- `typed`, `mir`, `metered_call` ~19% over ~2900 bodies, for a program that calls `fib`.
- Saving memos 3.4%.

Generated code:

- Native `fib` is 21 instructions. A 16-byte frame costs three (`mov x16,#0; movk; sub sp,sp,x16`)
  and each `sub` is followed by a redundant `mov`.
- Cranelift at `opt_level=speed` times the same as native: the shared LIR passes decide the speed.
  The gap to `-O2` is the middle-end (recursion inlining), not selection.
- No custom global allocator; `dist` is fat LTO without PGO. `Value::clone` runs 10.7M times cold.

## Where to speed up, in order

Measured on a binary built after decision 666 (lint only the checked package), so the figures
above already include it.

1. Prime lands (uncommitted in the default workspace: `driver/src/prime.rs`, `toolchain/prime.rs`;
   owner not confirmed). Then re-profile: the cold path loses the 316 library files.
2. Reachability, as a likely bug: `hello` still types ~2900 bodies (lint rules as Vun code and the
   comptime they run). Find what reaches them.
3. `Value::clone` (10.7M calls cold): the cause of the allocator pressure. Share (`Arc`,
   interning) or borrow in comptime before touching the allocator.
4. PGO plus BOLT of `vun`: no policy obstacle. mimalloc is pending the user's decision on
   decision 67 (no C code); not scheduled.
5. Queries that wait run other ready work, designed before built. Helping nests tasks on the
   waiter's stack: the helper can take work needing a claim held lower on its own stack or by the
   thread it waits on (false deadlock), and the depth is unbounded. `base/query/src/claim.rs` cycle
   detection must account for it; `vun-par`'s pool is the one `unsafe` zone (Miri run). Either
   restrict helping to tasks that cannot claim what the waiter holds, or make queries continuations.
6. A data-oriented parser, last: after priming only user files and edits pay for parsing, and it
   rewrites generated code (`grammar.vun`) and needs the fuzz runs. Re-measure `parsed` on a
   primed toolchain first.
7. Faster comptime (`metered_call` is 13 to 26% of a run): a register-based interpreter with
   superinstructions, or the native backend as a baseline JIT.
8. The edit loop: ~80 ms of re-checking and re-emitting for one constant; no per-function LIR code
   cache exists.

Generated-code items (another session works on optimizations): recursion inlining in LIR, cheaper
frame setup, redundant-move cleanup, peepholes declared as verified data.

## Frontier work

- TPDE (https://arxiv.org/abs/2505.22610, CGO 2026): one analysis pass, then one pass that selects,
  allocates and encodes; 8 to 24x faster than LLVM `-O0` at similar run speed; code 43 to 49%
  larger. Copy-and-patch is still faster on x86-64. Vun's native backend is in this class.
- Zig incremental compilation: function-level dependencies and in-place binary patching, millisecond
  rebuilds claimed [unverified: the sources are aggregator pages; read the original post].
- simdjson-style staged parsing (https://arxiv.org/abs/1902.08318): a SIMD structural pass over
  64-byte chunks, then a walk of the index. A SIMD C lexer reports 12x over GCC's
  (https://repository.tudelft.nl/file/File_0dd21f86-ee80-49e2-b4d1-bdb56f84f265).
- Data-oriented frontends: Carbon, Zig, Roc. No throughput figures found.

Not used yet [unchecked in the code]: zero-copy memory-mapped memos, a heap snapshot of the primed
library, e-graph rewriting in the LIR mid-end, copy-and-patch for comptime.
