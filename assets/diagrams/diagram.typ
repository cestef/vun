// Diagrams in the brand's style: boxes with labels, arrows between them, stages in a row, ranks
// of peers stacked, spans of code picked out. Every function takes a theme, `theme(name)`, made
// from a scheme of `palette.toml`; lengths are shares of `unit`, the size of the labels.
//
// Imported from the repository's root, `/assets/diagrams/diagram.typ`, by the diagram cards and
// the whitepaper.

#let palette = toml("../palette.toml")

// The scheme `name` and the size everything is drawn at.
#let theme(name, unit: 12pt) = (..palette.at(name), unit: unit)

#let font = "JetBrains Mono"

// Text in the face, `role` naming a color of the scheme.
#let label(t, body, role: "text", size: 1, weight: "regular") = text(
  font: font, size: t.unit * size, fill: rgb(t.at(role)), weight: weight, body,
)

// The stroke of the lines drawn in `role`.
#let pen(t, role: "muted", dash: none) = (paint: rgb(t.at(role)), thickness: t.unit * 0.1, dash: dash, cap: "round")

// A box around `body`, and `note` under it in the muted color. `kind`: "plain" on the surface,
// "accent" outlined in the accent, "faint" dashed for what is planned.
#let node(t, body, note: none, kind: "plain") = {
  let role = if kind == "accent" { "accent" } else { "muted" }
  let dash = if kind == "faint" { "dashed" } else { none }
  box(
    stroke: pen(t, role: role, dash: dash),
    radius: t.unit * 0.5,
    inset: (x: t.unit * 0.9, y: t.unit * 0.6),
    fill: if kind == "accent" { rgb(t.echo) } else { none },
    stack(
      spacing: t.unit * 0.4,
      label(t, body, role: if kind == "faint" { "muted" } else { "ink" }, weight: "bold"),
      ..if note != none { (label(t, note, role: "muted", size: 0.8),) },
    ),
  )
}

// An arrow `length` long towards `dir` (ltr, rtl, ttb or btt), with `note` beside it.
#let arrow(t, length: 3, dir: ltr, note: none, role: "muted") = {
  let l = t.unit * length
  let head = t.unit * 0.45
  let horizontal = dir == ltr or dir == rtl
  let (w, h) = if horizontal { (l, head * 2) } else { (head * 2, l) }
  // The tip and the two ends of the head, as if pointing right; turned for the others.
  let point(x, y) = if dir == ltr { (x, y) } else if dir == rtl { (l - x, y) } else if dir == ttb {
    (y, x)
  } else { (y, l - x) }
  let shaft = curve(stroke: pen(t, role: role), curve.move(point(0pt, head)), curve.line(point(l, head)))
  let tip = curve(
    stroke: pen(t, role: role),
    curve.move(point(l - head, 0pt)),
    curve.line(point(l, head)),
    curve.line(point(l - head, head * 2)),
  )
  let drawn = box(width: w, height: h, place(shaft) + place(tip))
  if note == none { drawn } else if horizontal {
    stack(spacing: t.unit * 0.3, align(center, label(t, note, role: "muted", size: 0.7)), drawn)
  } else {
    stack(dir: ltr, spacing: t.unit * 0.4, drawn, align(horizon, label(t, note, role: "muted", size: 0.7)))
  }
}

// `items` joined by arrows towards `dir`, each arrow pointing `back` against it when set; an item
// is content, or a dictionary of `node`'s arguments.
#let pipeline(t, items, dir: ltr, back: false, length: 2.2, note: none) = {
  let drawn = items.map(i => if type(i) == dictionary { node(t, i.body, ..i.at("args", default: (:))) } else { i })
  let towards = if back { (ltr: rtl, rtl: ltr, ttb: btt, btt: ttb).at(repr(dir)) } else { dir }
  let joined = drawn.slice(1).fold((drawn.first(),), (acc, i) => acc + (align(horizon + center, arrow(t, length: length, dir: towards, note: note)), i))
  stack(dir: dir, spacing: t.unit * 0.5, ..joined.map(j => align(center + horizon, j)))
}

// Ranks of peers stacked, the first at the bottom, each a row of nodes under its name; `rule`
// says what the order means, beside an arrow up the side.
#let ranks(t, ranks, rule: none) = {
  let row(names) = stack(dir: ltr, spacing: t.unit * 0.6, ..names.map(n => node(t, n)))
  let rows = ranks.rev().map(row)
  let column = stack(spacing: t.unit * 0.6, ..rows.map(r => align(center, r)))
  if rule == none { column } else {
    context {
      let height = measure(column).height
      stack(dir: ltr, spacing: t.unit * 1.2, column, arrow(t, length: height / t.unit, dir: btt, note: rule))
    }
  }
}

// A line of `code` with the characters `from` to `to` (from 0, the end excluded) underlined in
// the accent and `note` under them.
#let span(t, code, from, to, note: none) = context {
  let set-code(body) = text(font: font, size: t.unit * 1.1, fill: rgb(t.ink), ligatures: false, features: (calt: 0), body)
  let before = measure(set-code(code.slice(0, from))).width
  let width = measure(set-code(code.slice(from, to))).width
  let tick = t.unit * 0.4
  let bracket = curve(
    stroke: pen(t, role: "accent"),
    curve.move((0pt, 0pt)),
    curve.line((0pt, tick)),
    curve.line((width, tick)),
    curve.line((width, 0pt)),
  )
  // Left aligned, so the underline stays under its characters wherever the span is placed.
  set align(left)
  stack(
    spacing: t.unit * 0.3,
    set-code(code),
    pad(left: before, stack(
      spacing: t.unit * 0.4,
      box(width: width, height: tick, place(bracket)),
      ..if note != none { (label(t, note, role: "accent", size: 0.8),) },
    )),
  )
}

// A card holding `body` on the scheme's surface, `title` over it. With a `radius` the card is a
// rounded surface with a hairline edge, and the page around it is left clear.
#let card(t, title, body, radius: 0pt) = {
  let content = stack(spacing: t.unit * 2.2, label(t, title, role: "muted", size: 1.1), body)
  if radius == 0pt {
    set page(width: auto, height: auto, margin: t.unit * 3, fill: rgb(t.background))
    content
  } else {
    set page(width: auto, height: auto, margin: 0pt, fill: none)
    box(fill: rgb(t.background), radius: radius, stroke: 1pt + rgb(t.echo), inset: t.unit * 3, content)
  }
}
