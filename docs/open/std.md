# Open: `std` and the corpus

## Corpus (`cargo xtask corpus`)

- `std.Dyn` (`corpus/planned/types/dynamic.vun`, plus the `Encoder` `meta/derive.vun` waits for).
- `std.mem.copy` takes `&mut` slices: an `&out` destination needs the callee's initialization
  tracked through the unsafe copy.
- The grammar generator reads a PascalCase table name as "an enum" (`TokenKind` holds
  `#[lint.allow]`); the table's shape could say it.

## Library

- `task.Pool`, `testing.Sim`, `Str` and collections, generic bodies over `$A: Allocator`.
- `std.os.entropy` reports `unsupported` on Windows until the runtime links `bcryptprimitives`
  (`ProcessPrng`).
- A stable sort (needs scratch or an allocator), a pattern-defeating quicksort, `BTreeMap`
  (`SortedMap` is O(n) insert), `std.os.{env,process,net}` and directories, `std.json` reading
  beyond the pull reader.
- `std.math`: correctly rounded `exp`/`ln`/`sin`; `f16` transcendentals round twice.
- Iterators: `Iter.sum` (a generic `+` of a range type needs a design), `rev`, `peekable`;
  `Vec.drain`, `Vec.split_off`.
- A helper in `std.int` for an arithmetic shift or sign mask (signed bit tricks need `.wrap` round
  trips).
- `int.does_not_fit` help says "widen `u64`" for other run-time types; it should name `.to(T)` or
  `.wrap(T)`.
- A `mut x := e` takes the range of `e`, so `x -= 1` traps at its low end: annotate it.
- Plans (`vun build` on a package): `flash`, `test_on`, dependency parameters.
