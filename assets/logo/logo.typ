// The logo drawn in Typst for the cards, in the coordinates of `wordmark/*.svg`: a view box of
// 380 by 200, strokes 40 wide with round ends.

#let view = (width: 380, height: 200)
#let weight = 40

// The control distance of a cubic Bézier approximating a quarter circle, per unit of radius.
#let kappa = 0.5523
#let radius = 50

#let pen(color, unit) = (paint: rgb(color), thickness: weight * unit, cap: "round", join: "round")

#let v(unit) = {
  let p(x, y) = (x * unit, y * unit)
  (curve.move(p(40, 40)), curve.line(p(90, 160)), curve.line(p(140, 40)))
}

#let un(unit) = {
  let p(x, y) = (x * unit, y * unit)
  let k = kappa * radius
  (
    curve.move(p(140, 40)),
    curve.line(p(140, 110)),
    curve.cubic(p(140, 110 + k), p(190 - k, 160), p(190, 160)),
    curve.cubic(p(190 + k, 160), p(240, 110 + k), p(240, 110)),
    curve.line(p(240, 40)),
    curve.move(p(240, 40)),
    curve.line(p(240, 160)),
    curve.move(p(240, 90)),
    curve.cubic(p(240, 90 - k), p(290 - k, 40), p(290, 40)),
    curve.cubic(p(290 + k, 40), p(340, 90 - k), p(340, 90)),
    curve.line(p(340, 160)),
  )
}

// The wordmark `height` tall, its view box included.
#let wordmark(height, accent: "#D46287", ink: "#24211D") = {
  let unit = height / view.height
  box(width: view.width * unit, height: height, {
    place(curve(stroke: pen(accent, unit), ..v(unit)))
    place(curve(stroke: pen(ink, unit), ..un(unit)))
  })
}

// The v alone, drawn at `unit` per view box unit from the view box's origin.
#let mark(unit, color) = curve(stroke: pen(color, unit), ..v(unit))

// The extent of the v with its stroke, from the view box's origin.
#let mark-box = (left: 20, top: 20, width: 140, height: 160)
// The extent of the wordmark with its strokes, from the view box's origin.
#let wordmark-box = (left: 20, top: 20, width: 340, height: 160)
