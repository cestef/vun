# Open: running tests

Spec 11 is the reference.

- Properties lend only shared borrows (`&T`, `&[T]`); a slice holds at most
  `std.testing.property.longest` (32) items.
- `vun fuzz` (387) keeps its streams in memory: no corpus in `fuzz/<test path>/`, no traced
  comparisons, so a byte among a long stream is found slowly.
- Simulations (386): no simulated network or disk, nor choices drawn by the tasks themselves.
- Benchmarks (384, 600 to 602): no interpreter operation count and no Cortex-M cycle counter for
  `--measure instructions`; a sample is a whole run of the program, so a benchmark's samples cost
  `--samples` times 500 ms.
- `vun cover` (580, 645): no branch counts, no workspace of packages in one lcov file, `--jobs` is
  not honoured (one process runs every test), a test that ends the process writes no counts,
  and a line reports the most-run block written on it, not the lines a multi-line expression
  spans. Regions still share counts modulo `lang.cover.size`; the report says how many do, and
  numbering them densely needs the program known when a function lowers.
- `#[test.traps()]` (640) does not tell which trap ended the test: the program's table holds a
  site's text, not a typed kind.
- `#[test.fuel(n)]` means something only where a mode counts steps (`interp`, not built).
- Loose files stay in no namespace (`test.no_namespace`); recommended to keep, the owner may
  overrule.
- `vun debug smith` (see spec 11): not generated yet are generic iterators, coroutines that
  return or are cancelled, tasks, `unsafe`, and layout attributes. Untriaged: a text-fuzz crash
  at `compiler/low/lower/src/func/constant.rs:38` (`f16` constants; `int` slices past the type's
  size, so a `Constant::Float` met a smaller type). The `program` fuzz target found no input in
  7 minutes; run it with `ASAN_OPTIONS=detect_leaks=0` and `-rss_limit_mb` raised, one session
  grows past the default limit.
- `vun test main.vun` is `test.no_namespace` for a program's root file: a package's namespaces are the
  files its root namespace names, which the `hello` template's `main.vun` is not, so the template
  has nowhere to put a test.
