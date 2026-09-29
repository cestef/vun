// Stickers: each figure of `figures.typ` cut round at a border in its scheme's background, in
// every scheme of `palette.toml`, for printing services that take one image per sticker.
//
// Compiled by `cargo xtask typeset` at 72 pixels per inch, so a point is a pixel, once per
// variant declared at the end.

#import "../figures.typ": figure, names

#let palette = toml("../../palette.toml")

// The figure's height and the border round it, per unit of that height.
#let height = 600pt
#let border = 0.12

#let name = sys.inputs.at("figure", default: "calm")
#let scheme = sys.inputs.at("scheme", default: "light")

#set page(width: auto, height: auto, margin: 0pt, fill: none)

#figure(name, palette, scheme, height, border: height * border, cut: rgb(palette.at(scheme).background))

#metadata(palette.keys().map(s => names.map(f => (path: (s, f), inputs: (scheme: s, figure: f)))).flatten()) <variants>
