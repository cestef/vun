// The app icons: the v on a rounded tile, in every scheme of `palette.toml` and every size below.
//
// Compiled by `cargo xtask typeset` at 72 pixels per inch, so a point is a pixel, once per
// variant declared at the end.

#import "../../logo/logo.typ": mark, mark-box

#let palette = toml("../../palette.toml")

// The sizes, in pixels, a platform asks for: favicons, the Apple touch icon, web app manifests.
#let sizes = (16, 32, 48, 180, 192, 512, 1024)

#let scheme = palette.at(sys.inputs.at("scheme", default: "light"))
#let size = int(sys.inputs.at("size", default: "512")) * 1pt

// The tile's rounding and the v's height, per unit of the side; the v sits a little low, where
// its wide top balances its point.
#let rounding = 0.24
#let height = 0.5
#let drop = 0.02

#set page(width: size, height: size, margin: 0pt, fill: none)

#let unit = size * height / mark-box.height
#box(width: size, height: size, radius: size * rounding, fill: rgb(scheme.background),
  place(center + horizon, dy: size * drop,
    box(width: mark-box.width * unit, height: mark-box.height * unit,
      move(dx: -mark-box.left * unit, dy: -mark-box.top * unit, mark(unit, scheme.accent)))))

#metadata(palette.keys().map(s => sizes.map(n => (path: (s, str(n)), inputs: (scheme: s, size: str(n))))).flatten()) <variants>
