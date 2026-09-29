// The app icons: the v on a rounded tile, in every scheme of `palette.toml` and every size below;
// under `mascot/`, the mascot on the tile instead.
//
// Compiled by `cargo xtask typeset` at 72 pixels per inch, so a point is a pixel, once per
// variant declared at the end.

#import "../../logo/logo.typ": mark, mark-box
#import "../mascot.typ": mascot, width

#let palette = toml("../../palette.toml")

// The sizes, in pixels, a platform asks for: favicons, the Apple touch icon, web app manifests.
#let sizes = (16, 32, 48, 180, 192, 512, 1024)

#let name = sys.inputs.at("scheme", default: "light")
#let scheme = palette.at(name)
// The mascot's mood, drawn instead of the v, or none.
#let mood = sys.inputs.at("mascot", default: none)
#let size = int(sys.inputs.at("size", default: "512")) * 1pt

// The tile's rounding and the v's height, per unit of the side; the v sits a little low, where
// its wide top balances its point.
#let rounding = 0.24
#let height = 0.5
#let drop = 0.02
// The mascot's width, per unit of the side.
#let wide = 0.64

#set page(width: size, height: size, margin: 0pt, fill: none)

#let unit = size * height / mark-box.height
#let v = box(width: mark-box.width * unit, height: mark-box.height * unit,
  move(dx: -mark-box.left * unit, dy: -mark-box.top * unit, mark(unit, scheme.accent)))
#let figure = if mood == none { place(center + horizon, dy: size * drop, v) } else {
  place(center + horizon, mascot(name, mood, size * wide / width(1)))
}
#box(width: size, height: size, radius: size * rounding, fill: rgb(scheme.background), figure)

#let plain = palette.keys().map(s => sizes.map(n => (path: (s, str(n)), inputs: (scheme: s, size: str(n)))))
#let mascots = palette.keys().map(s => sizes.map(n => (path: ("mascot", s, str(n)), inputs: (scheme: s, size: str(n), mascot: "calm"))))
#metadata((plain + mascots).flatten()) <variants>
