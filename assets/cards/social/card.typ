// The social cards of the repository: the wordmark and the tagline, in every scheme of
// `palette.toml` and every format below, the logo enlarged behind them; under `mascot/`, the same
// with the mascot beside them.
//
// Compiled by `cargo xtask typeset` at 72 pixels per inch, so a point is a pixel, once per
// variant declared at the end.

#import "../../logo/logo.typ": wordmark, mark, mark-box
#import "../mascot.typ": mascot

#let palette = toml("../../palette.toml")

#let formats = toml("../formats.toml")

#let name = sys.inputs.at("scheme", default: "light")
#let scheme = palette.at(name)
// The mascot's mood, or none.
#let mood = sys.inputs.at("mascot", default: none)
#let format = formats.at(sys.inputs.at("format", default: "github"))

// Every length is a share of the shorter side, so the cards agree at every size.
#let side = calc.min(format.width, format.height) * 1pt
#let margin = side * 0.16

#set page(width: format.width * 1pt, height: format.height * 1pt, margin: margin, fill: rgb(scheme.background))
#set text(font: "JetBrains Mono", fill: rgb(scheme.text))

// The v, as tall as the shorter side and cut by the right and bottom edges.
#let echo = side / mark-box.height
#place(bottom + right, dx: margin + mark-box.width * echo * 0.3, dy: margin + mark-box.height * echo * 0.12,
  box(width: mark-box.width * echo, height: mark-box.height * echo,
    move(dx: -mark-box.left * echo, dy: -mark-box.top * echo,
      mark(echo, scheme.echo))))

#let size = side * 0.3
// The tagline, about 23 advances of the monospace face, fits between the margins.
#let tagline = calc.min(side * 0.047, (format.width * 1pt - 2 * margin) / 23)

#place(horizon + left, stack(
  spacing: side * 0.07,
  move(dx: -mark-box.left * size / 200, wordmark(size, accent: scheme.accent, ink: scheme.ink)),
  text(size: tagline)[A smol systems programming language.],
))
#place(bottom + left, text(size: side * 0.028, fill: rgb(scheme.muted),
  "github.com/cestef/vun"))
// The mascot in front of the echo: standing on the bottom margin of a wide card, as tall as the
// wordmark's letters; on a narrow one, where the tagline takes that room, peeking over the bottom
// edge with no more than its face, within the margin.
#if mood != none {
  if format.width > 1.5 * format.height {
    place(bottom + right, mascot(name, mood, side * 0.3))
  } else {
    let (tall, shown) = (side * 0.26, side * 0.14)
    place(bottom + right, dy: margin + tall - shown, mascot(name, mood, tall))
  }
}

#let plain = palette.keys().map(s => formats.keys().map(f => (path: (s, f), inputs: (scheme: s, format: f))))
#let beside = palette.keys().map(s => formats.keys().map(f => (path: ("mascot", s, f), inputs: (scheme: s, format: f, mascot: "glee"))))
#metadata((plain + beside).flatten()) <variants>
