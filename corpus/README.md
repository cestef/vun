# Corpus

Programs written before the compiler, to test the design (roadmap phase 0) and later the compiler.

- `accept/`: programs that must compile. Each is explained by the concept list and appendix B.
- `reject/`: programs that must fail with exactly the diagnostics annotated in them:
  `//~ id` on the offending line, or `//~^ id` for the line above (one `^` per line).

`cargo xtask corpus` runs both through `overt-harness`.
