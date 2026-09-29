// The social cards of the repository: the wordmark and the tagline, in every scheme of
// `palette.toml` and every format below, the logo enlarged behind them.
//
// Compiled by `cargo xtask typeset` at 72 pixels per inch, so a point is a pixel, once per
// variant declared at the end.

#import "../../logo/logo.typ": wordmark, mark, mark-box

#let palette = toml("../../palette.toml")

// The sizes, in pixels, of the places a card is shown.
#let formats = (
  github: (width: 1280, height: 640),
  graph: (width: 1200, height: 630),
  banner: (width: 1500, height: 500),
  square: (width: 1080, height: 1080),
  portrait: (width: 1080, height: 1350),
)

#let scheme = palette.at(sys.inputs.at("scheme", default: "light"))
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
// The tagline's longest line, about 20 advances of the monospace face, fits between the margins.
#let tagline = calc.min(side * 0.047, (format.width * 1pt - 2 * margin) / 20)

#place(horizon + left, stack(
  spacing: side * 0.07,
  move(dx: -mark-box.left * size / 200, wordmark(size, accent: scheme.accent, ink: scheme.ink)),
  text(size: tagline)[A systems programming language \ with nothing behind the scenes.],
))
#place(bottom + left, text(size: side * 0.028, fill: rgb(scheme.muted),
  ("x86-64", "arm64", "Thumb-2", "wasm32", "C").join(h(side * 0.025))))

#metadata(palette.keys().map(s => formats.keys().map(f => (path: (s, f), inputs: (scheme: s, format: f)))).flatten()) <variants>
