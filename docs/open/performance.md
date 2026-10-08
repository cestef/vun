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
- Warm builds emit every function again (`emit`, ~5 ms for `tour`): no code cache per LIR function.
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
- `vun debug profile` on `am3.wg` (2026-10-08, hello, warm unless said): 332 M instructions, 47 ms.
  Where it goes, in order of what to try:
  1. Parsing: `parsed` is 50% of the warm time (24 ms for 23 files) and `Parser::read_root`
     runs 1.0M times at 13.7% own, `taking` 9%, `Positions::filter` 6%. Warm still parses the
     library because parse trees are not saved; saving them or making `read_root` cheaper
     per call (45 instructions each, 1.0M calls) is the largest single win.
  2. `blake3::portable::compress_in_place` is 7.6% warm and 2.3% cold: `blake3` is built with
     `pure` (decision 67, no C or assembly), which also drops its SIMD paths (NEON on aarch64,
     AVX2 on x86). Dropping `pure` buys most of that back and costs a C compiler; the owner's
     call. Hashing fewer bytes (what is hashed on a warm run) costs nothing.
  3. Cold: `Table<Parsed>::slot` 2.4M calls at 140 instructions each (3.9%), `Table<Typed>::slot`
     and `Table<DeclValue>::slot` the same shape; SipHash `write` 2.3% (a `std` `HashMap` with the default hasher
     somewhere hot; `foldhash` is used elsewhere); `Value::clone` 10.7M calls and its drop 6.0M
     (4.2% together).
  4. Time cold (hello 534 ms): `parsed` 25%, `metered_call` 13%, `typed` 12.5%, `mir` 9%,
     `save memos` 9%, waiting for claims 6%. Comptime-heavy (`corpus/bench/comptime.vun`)
     is 857 ms, 26% in `metered_call` over 31 000 calls.
  `Vec<u8>` encoded byte by byte (3.8% cold): fixed, cold `hello` 8.70 G to 8.25 G instructions.
  Cold counts vary ~1% between runs (warm 0.0%); find why (a time- or address-dependent order).
- Generated code (`run-*`, am3): `fib(34)` native 126 M instructions (20.8 ms) in 1.9M calls,
  cranelift 286 M in 27.7M calls, so native inlines the recursion about nine times; `sieve`
  8.4 M (2.3 ms), 19% of it `memcpy` of the array initializer; `mix` 87 ms. Native binaries
  carry no function names (`f129`), so `profile` cannot name a hot function there: emit symbols
  for local functions. Nothing yet compares the backends instruction for instruction in one
  report; run the same `run-*` with `--backend`.
