// The compiler's stages, from source to a linked program: what each makes, and the `vun debug`
// view that shows it.

#import "diagram.typ": node, pipeline

#let stages = (
  (body: "source", args: (note: ".vun")),
  (body: "tokens", args: (note: "debug tokens")),
  (body: "cst", args: (note: "debug cst")),
  (body: "hir", args: (note: "debug hir")),
  (body: "types", args: (note: "debug types")),
  (body: "mir", args: (note: "debug mir")),
  (body: "lir", args: (note: "debug lir")),
  (body: "code", args: (note: "debug code")),
  (body: "program", args: (note: "linked", kind: "accent")),
)

// In two rows when `wide` is false, the second continuing below the first.
#let diagram(t, wide: true) = if wide {
  pipeline(t, stages)
} else {
  let half = calc.ceil(stages.len() / 2)
  stack(spacing: t.unit * 1.6, pipeline(t, stages.slice(0, half)), pipeline(t, stages.slice(half)))
}
