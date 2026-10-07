# Open: `std` and the corpus

## Corpus (`cargo xtask corpus`)

- `std.mem.copy` takes `&mut` slices: an `&out` destination needs the callee's initialization
  tracked through the unsafe copy.
- The grammar generator reads a PascalCase table name as "an enum" (`TokenKind` holds
  `#[lint.allow]`); the table's shape could say it.

## Library

- `task.Pool`, `testing.Sim`, `Str` and collections, generic bodies over `$A: Allocator`.
- `std.os.entropy` reports `unsupported` on Windows until the runtime links `bcryptprimitives`
  (`ProcessPrng`).
- A stable sort (needs scratch or an allocator), a pattern-defeating quicksort,
  `std.os.{env,process,net}` and directories, `std.json` reading beyond the pull reader. `BTreeMap`
  has no entry value to change in place beyond `get_or_insert`.
- `std.math`: correctly rounded `exp`/`ln`/`sin`; `f16` transcendentals round twice.
- A `mut x := e` takes the range of `e`, so `x -= 1` traps at its low end: annotate it.
- Plans (`vun build` on a package): `flash`, `test_on`, dependency parameters.
