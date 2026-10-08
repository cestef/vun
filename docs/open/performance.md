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
- `Concrete::propagate` reaches its fixed point one constant level per scan of the body
  (`concrete/mod.rs`); a worklist over writes would do it in one. It does not show in the
  profile of a warm `vun test lib/std`; measure a cold run before changing it.
