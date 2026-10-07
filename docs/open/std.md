# Open: `std`

- `task.Pool`, `testing.Sim`, `Str` and collections, generic bodies over `$A: Allocator`.
- `std.os.entropy` reports `unsupported` on Windows until the runtime links `bcryptprimitives`
  (`ProcessPrng`).
- A stable sort (needs scratch or an allocator), a pattern-defeating quicksort,
  `std.os.{env,process,net}` and directories, `std.json` reading beyond the pull reader. `BTreeMap`
  has no entry value to change in place beyond `get_or_insert`.
- `std.math`: correctly rounded `exp`/`ln`/`sin`; `f16` transcendentals round twice.
- A `mut x := e` takes the range of `e`, so `x -= 1` traps at its low end: annotate it. A lint
  for it needs the type of the local, which the lint walker (syntax only) does not see.
- Plans (`vun build` on a package): `flash`, `test_on`, dependency parameters.
