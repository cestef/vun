# Open: language and compiler front

- An `impl` member may leave its types to the requirement (403, 404, 405). Still written: a member
  with another number of parameters than its requirement (`Concat.concat` lacks `$R`, `Iterate` for
  a slice takes `&[T]` where the requirement says `Self`), a coroutine's result, and a parameter the
  call cannot say before the closure is typed (a `fold` accumulator nothing expects a result of).
- A `$`-generic C import (a pack inferred per call) is typed nowhere: such calls stay silently
  untyped (285, 286).
- A pattern-only form where a value is expected is never reported: `&xs[..]` parses its subscript
  as a `Rest`, HIR lowering makes it `ExprKind::Error` (`Lower::expr_kind`, no `DiagCx`), and typing
  calls it a compiler bug. Either the grammar's operand contexts keep `Rest` to patterns, or
  lowering reports what it turns into `Error`.
- Evaluator: loans inside aggregate arguments, memory per nesting level.
- `vun-sema` is one dependency cycle (about 12k lines): evaluation needs typed bodies and typing
  needs evaluation. Splitting means owning traits in the lower part, as `Evaluator` does (`typeck`
  could reach `asm` through a registered checker).
- `render/sarif`; `DiagId` and pass ids are strings.
