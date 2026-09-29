// The figures of the emoji and the stickers: the mascot in each mood, the v and the wordmark,
// each drawn to a height, tight to its extent, and optionally cut round at a border.

#import "../logo/logo.typ": v, un, pen, weight, mark-box, wordmark-box
#import "mascot.typ": mascot, moods, info

// The logo's figures: their extent in the view box and their strokes, each with the role of
// `palette.toml` that paints it.
#let logos = (
  v: (box: mark-box, strokes: ((v, "accent"),)),
  vun: (box: wordmark-box, strokes: ((v, "accent"), (un, "ink"))),
)

#let names = moods + logos.keys()

// Its width over its height.
#let aspect(name) = if name in logos { logos.at(name).box.width / logos.at(name).box.height } else { info.aspect }

// The strokes of `figure` in `scheme` at `unit` per view box unit, widened by `grow` on each side
// and painted `paint` instead when given.
#let strokes(figure, scheme, unit, grow: 0pt, paint: none) = {
  for (segments, role) in figure.strokes {
    let color = if paint == none { scheme.at(role) } else { paint }
    let stroke = pen(color, unit)
    place(curve(stroke: (..stroke, thickness: stroke.thickness + 2 * grow), ..segments(unit)))
  }
}

// The figure `name` in the scheme named `scheme` of `palette`, `height` tall, its extent grown by
// `border` on every side and filled there with `cut`, the edge a die cuts along.
#let figure(name, palette, scheme, height, border: 0pt, cut: none) = {
  let colors = palette.at(scheme)
  if name in logos {
    let logo = logos.at(name)
    let unit = height / logo.box.height
    box(width: logo.box.width * unit + 2 * border, height: height + 2 * border,
      // Anchored at the top left, whatever alignment the box is set in.
      place(top + left, dx: border - logo.box.left * unit, dy: border - logo.box.top * unit, {
        if cut != none { strokes(logo, colors, unit, grow: border, paint: cut) }
        strokes(logo, colors, unit)
      }))
  } else {
    let (tl, tr, br, bl) = info.corners.map(r => r * height + border)
    box(inset: border, fill: cut, radius: (top-left: tl, top-right: tr, bottom-right: br, bottom-left: bl),
      mascot(scheme, name, height))
  }
}
