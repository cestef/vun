// The mascot with the wordmark: beside it in a row, or above it in a stack, in every scheme of
// `palette.toml`.
//
// Compiled by `cargo xtask typeset` at 72 pixels per inch, so a point is a pixel, once per
// variant declared at the end.

#import "../../logo/logo.typ": wordmark, view, weight
#import "../mascot.typ": mascot

#let palette = toml("../../palette.toml")
#let name = sys.inputs.at("scheme", default: "light")
#let scheme = palette.at(name)
#let layout = sys.inputs.at("layout", default: "row")

// The wordmark's height, its view box included; the letters span all of it but the margin of
// half a stroke above and below, and the mascot stands as tall as they do.
#let size = 240pt
#let unit = size / view.height
#let margin = weight / 2 * unit
#let tall = size - 2 * margin

#set page(width: auto, height: auto, margin: 48pt, fill: rgb(scheme.background))

#let word = wordmark(size, accent: scheme.accent, ink: scheme.ink)
#let figure = pad(y: margin, mascot(name, "glee", tall))

#if layout == "row" {
  stack(dir: ltr, spacing: 2 * margin, figure, word)
} else {
  stack(spacing: 0pt, align(center, figure), word)
}

#metadata(palette.keys().map(s => ("row", "stack").map(l => (path: (s, l), inputs: (scheme: s, layout: l)))).flatten()) <variants>
