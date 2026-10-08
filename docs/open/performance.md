# Open: compile speed

- A cold `vun test lib/std` takes ~4.5 s on `am3.wg`, spread over `mir`, `typed`, lint rules run as
  Vun code (18 000 compile-time calls) and writing the memos (0.6 s).
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
