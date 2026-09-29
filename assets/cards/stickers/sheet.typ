// A sheet of every sticker of `card.typ` on A4, in every scheme of `palette.toml`, as vector
// PDF for printing.
//
// Compiled by `cargo xtask typeset` to PDF, once per variant declared at the end.

#import "../figures.typ": figure, logos
#import "../mascot.typ": moods

#let palette = toml("../../palette.toml")

// The stickers' height, the border round them, per unit of that height, and the room between.
#let height = 38mm
#let border = 0.12
#let gutter = 8mm

#let scheme = sys.inputs.at("scheme", default: "light")

#set page(paper: "a4", margin: 15mm, fill: none)

#let sticker(name) = figure(name, palette, scheme, height, border: height * border, cut: rgb(palette.at(scheme).background))

// The moods in a grid, the logo's figures in a row under them.
#align(center + horizon, stack(
  spacing: gutter,
  grid(columns: 3, gutter: gutter, ..moods.map(sticker)),
  stack(dir: ltr, spacing: gutter, ..logos.keys().map(sticker)),
))

#metadata(palette.keys().map(s => (path: (s,), inputs: (scheme: s)))) <variants>
