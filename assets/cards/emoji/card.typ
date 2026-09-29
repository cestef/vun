// Emoji for chat: each figure of `figures.typ` centered on a square with no background, in every
// scheme of `palette.toml`, at the size chat platforms ask for and a master for them to scale.
//
// Compiled by `cargo xtask typeset` at 72 pixels per inch, so a point is a pixel, once per
// variant declared at the end.

#import "../figures.typ": figure, names, aspect

#let palette = toml("../../palette.toml")

// Discord and Slack take 128 pixels; the master is for anything that asks larger.
#let sizes = (128, 512)
// The room kept clear round the figure, per unit of the side.
#let clear = 0.04

#let name = sys.inputs.at("figure", default: "calm")
#let scheme = sys.inputs.at("scheme", default: "light")
#let side = int(sys.inputs.at("size", default: "128")) * 1pt

#set page(width: side, height: side, margin: side * clear, fill: none)

#let room = side * (1 - 2 * clear)
#let height = calc.min(room, room / aspect(name))
#place(center + horizon, figure(name, palette, scheme, height))

#metadata(palette.keys().map(s => sizes.map(n => names.map(f =>
  (path: (s, str(n), f), inputs: (scheme: s, size: str(n), figure: f))))).flatten()) <variants>
