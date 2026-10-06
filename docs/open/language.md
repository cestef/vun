# Open: language and compiler front

- `vun-sema` is one dependency cycle (about 12k lines): evaluation needs typed bodies and typing
  needs evaluation. Splitting means owning traits in the lower part, as `Evaluator` does (`typeck`
  could reach `asm` through a registered checker).
