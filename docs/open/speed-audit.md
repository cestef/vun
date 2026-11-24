# Open: speed audit (2026-10-08)

Measured on `am3`, pinned, from the benchmark of `scripts/bench-compare` and `vun debug profile`.
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
- mimalloc is already the global allocator (`vun-cli`, feature `mimalloc`; 7.6% of cold instructions
  are `libmimalloc-sys`). `dist` is fat LTO without PGO. Callgrind of a cold build: `Value::clone` is
  2.5M runs and 1.1% of instructions (the 10.7M in `performance.md` is another workload).

## Where to speed up, in order

Measured on a binary built after decision 666 (lint only the checked package), so the figures
above already include it.

1. Priming is built (`compiler/tools/driver/src/prime.rs`, `vun toolchain prime`, decision 667).
   Left: re-profile a cold path on a primed toolchain, and prime the common instantiations.
2. Reachability (found, fixed for methods): `hello` typed 2851 bodies on a cold run (single thread,
   `VUN_LOG=vun_query=trace`, which names each query's parent and depth), 74% of them under a
   `resolved` frame, a witness made for an impl. A method lookup (`Env::methods`) made the witness
   of every impl covering the type to see whether it declares the name, so `hello` typed the `json`,
   `hash`, `debug` and `property` impls of every integer type. `ImplDecl::may_have` now reads the
   impl as written (its body's declarations, then its interface's defaults) and makes no witness
   for an impl that cannot have the name: 2851 typed bodies fell to 1203. Of what is left, 324
   bodies are the `lang.int` operator impls per integer range; attribute the rest with the same
   trace. The typed bodies fell, but callgrind of a cold `hello` on a dev build did not follow
   (10.13 G before, 10.36 G after, one thread): find where the instructions go before counting
   this as a speedup.
3. Query-table overhead, engine-wide so it helps warm and edit too: `Db::table_of` goes through a
   thread-local `RefCell<HashMap>` on every lookup (877k calls) and papaya slots are 9% of cold
   instructions (`Table<Parsed>::slot` 2.9%). Measured on a dev build: the thread-local lookup is
   0.6% of a cold `hello`, so replacing it alone is not worth it; a cold run's 12% in
   `blake3_hash_many_neon` is the hash of the compiler executable (`persist/saved.rs`), which a
   debug binary of 286 MB inflates. `Value::clone` is only 1.1%: dropped.
4. BOLT of `vun`: measured, no gain, not built into the release. On am3 (arm64, llvm-bolt 22.1.8, the
   Fedora package unpacked beside the checkout), the PGO `dist` `vun` linked with `--emit-relocs` and
   unstripped, instrumented, run over the PGO workloads, then rewritten with `-reorder-blocks=ext-tsp
   -reorder-functions=cdsort -split-functions -icf=1`: taken branches fell 25% in BOLT's own
   statistics, but cold `build-hello`, `build-tour` and `check-std` and warm builds stayed within the
   noise (3 to 5%, interleaved runs). PGO and fat LTO already lay the hot code out. mimalloc v3
   measured the same as v2.
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
8. Machine code is kept per function (decision 669: warm `build-hello` 248 to 160 M instructions).
   Left: key it before legalizing, which still runs for the whole unit (~2.5 ms warm `fib`), and
   ship it with a primed toolchain.

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
