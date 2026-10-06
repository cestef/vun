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
- `#[test.fuel(n)]` means something only where a mode counts steps (`interp`, not built).
- Loose files stay in no namespace (`test.no_namespace`); recommended to keep, the owner may
  overrule.
