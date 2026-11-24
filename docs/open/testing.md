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
  return or are cancelled, tasks, `unsafe`, and layout attributes. A text-fuzz crash in `compiler/low/lower/src/func/constant.rs` (an `f16` constant, or an `int` slice past
  the type's size, so a `Constant::Float` met a smaller type) is fixed at its source: a float
  constant is encoded only into a float of its format, anything else is `Unlowered::unsupported`.
  The crash input was not kept, so no regression test exists; a rerun of the `lower` target is
  the check. The `program` fuzz target found no input in
  7 minutes; run it with `ASAN_OPTIONS=detect_leaks=0` and `-rss_limit_mb` raised, one session
  grows past the default limit.

## Conventions adopted from the sibling projects

What is left of the adoption (the rules are in `CLAUDE.md`):

- Binder's `Failure::Errors(Vec<String>)` is clang's own wording, a legitimate edge. The flattening is
  one site, `compiler/tools/binder/src/remote.rs`, after a component `Fault` (host-side, not
  encodable). Fix: `Binder::bind` returns `enum BindError { Failed(Failure), Component(Fault) }`
  with `source()`, and the CLI attaches the `Fault` through `Diagnostic::caused_by`.
- Kept on purpose: the diagnostic group `types` keeps its name (`KEPT` in `xtask/src/names.rs`, its
  ids are public); `Number::parse` and `json::parse` borrow their input, so `FromStr` cannot
  express them; the comment ratchet baseline (`xtask/comments.vun`) is lowered with
  `cargo xtask comments --bless` as comments move onto the enclosing item's doc.
