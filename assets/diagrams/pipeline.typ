// The compiler's stages, from source to a linked program: what each makes, and the `vun debug`
// view that shows it.

#import "diagram.typ": node, pipeline

#let stages = (
  (body: "source", args: (note: ".vun")),
  (body: "tokens", args: (note: "debug dump tokens")),
  (body: "cst", args: (note: "debug dump cst")),
  (body: "hir", args: (note: "debug dump hir")),
  (body: "types", args: (note: "debug dump types")),
  (body: "mir", args: (note: "debug dump mir")),
  (body: "lir", args: (note: "debug dump lir")),
  (body: "code", args: (note: "debug dump code")),
  (body: "program", args: (note: "linked", kind: "accent")),
)

// In two rows when `wide` is false, the second continuing below the first.
#let diagram(t, wide: true) = if wide {
  pipeline(t, stages)
} else {
  let half = calc.ceil(stages.len() / 2)
  stack(spacing: t.unit * 1.6, pipeline(t, stages.slice(0, half)), pipeline(t, stages.slice(half)))
}
