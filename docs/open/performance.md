# Open: compile speed

- A cold `vun test lib/std` takes ~4.5 s on `am3.wg`, spread over `mir`, `typed`, lint rules run as
  Vun code (18 000 compile-time calls) and writing the memos (0.7 s for a 59 MB project file of
  `lib/std`). What the file holds is mostly the dependency edges: `typed` 1.2M reads, `metered_call`
  960k, `mir` 590k (`vun debug queries lib/std/std.vun`); the library-shared file is 0.4 MB, so
  nothing is encoded twice. The witness lookup read one `Impls` and `Derives` per file of the
  toolchain for every type (2.4M reads, 367 ms own); it reads one memo per package now
  (`PackageImpls`, 55k reads, 57 ms).
  `VUN_LOG=vun_lir_passes::promote=trace` reports each slot promotion's cost.
- Truly cold (empty store), threads wait ~1.1 s on the same `std` claims, once per toolchain.
- A cold run saves the shared file through one writer (~37 ms for 4.3 MB): values encoded, copied
  into each enclosing blob, hashed whole.
- A shared file stays as its first writer left it: later projects recompute the library instances
  it lacks (a few ms each) and keep them in their own files.
- A build after an edit instantiating library generics parses those files again (~18 ms for 31
  files of `tour`): parse trees are traced, not saved.
- `vun-par` callers wait on uneven items (2 to 25 ms). Parse is ~11 MiB/s, the lexer ~145; the
  profile is flat (per-token decision over frames and specs positions, rebuilt at each check).
- A comptime-heavy benchmark is wanted before perf audits. The stack a nested compile-time call
  holds is still ~1.7 KB in `fetch`, `get` and `compute` of `MeteredCall` (`--emit asm`, `sub sp`;
  `Cx::computed` went from 960 to 496 bytes).
- A memo computed while no target was chosen (`vun build main.vun --target nope`) was reused by a
  later session that had one: `decl_value(lang.target.current)` came back untargeted
  (`VUN_QUERY_CHECK=1` panics in `reuse.rs`). A session whose target is not chosen now keeps no
  queries (`Session::untargeted`), which hides it; the cause is a read of the unset `TargetOf`
  input, saved with the memo but not held against the value set later.
- Cold `vun test lib/std` on `am3.wg` (empty `VUN_STORE`): `Body::propagate` is 3.4% of the samples
  with its callees and 0.07% on its own, so the repeated scans cost nothing; the rest is the
  compile-time calls `fold` evaluates, which `concrete` (2.6 s total across threads) holds.
- `vun debug profile` on `am3.wg` (2026-10-08, `hello`): a first check from nothing is 4.7 G
  instructions, 600 ms; on a primed toolchain (`vun toolchain prime`, decision 667) 0.7 G, 70 ms;
  warm 0.13 G, 25 ms. Open, by size:
  1. The shipped layer is 75 MB (10 to 13 MB compressed): it is every non-generic body of `lang`,
     `std` and `targets`. Generic instantiations a program needs are computed and saved with the
     project. Wanted: a small second layer from sample programs for the common instantiations
     (`fmt`, collections), and memos for the other targets (wasm, bare): each target and set of
     checks is a file of its own, primed for the host only. The playground keeps nothing between
     visits unless it ships the file too.
  2. Cold parse (a run with no memos, a library the toolchain does not ship, a package edited):
     ~7000 instructions a token, 27% of a cold run, flat (`Spec` built 4.6M times, kind-set
     tests, `decide` walking the frame stack). Only caching decisions across tokens would
     change it.
  3. Cold counts vary ~1% between runs (warm 0.0%); find why.
  4. `blake3` hashes with NEON for long inputs (decision 665); single blocks stay portable.
- Generated code (`run-*`, am3): `fib(34)` native 126 M instructions (20.8 ms) in 1.9M calls,
  cranelift 286 M in 27.7M calls, so native inlines the recursion about nine times; `sieve`
  8.4 M (2.3 ms, measured at 400 000 flags; the benchmark sieves 4 000 000 now), 19% of it `memcpy` of the array initializer; `mix` 87 ms. Native binaries
  carry no function names (`f129`), so `profile` cannot name a hot function there: emit symbols
  for local functions. Nothing yet compares the backends instruction for instruction in one
  report; run the same `run-*` with `--backend`.
