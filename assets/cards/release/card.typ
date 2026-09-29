// A card per release of `releases.toml`: the wordmark, the version and its date, its highlights,
// the mascot cheering, the logo enlarged behind; in every scheme of `palette.toml` and every
// format of `formats.toml`.
//
// Compiled by `cargo xtask typeset` at 72 pixels per inch, so a point is a pixel, once per
// variant declared at the end.

#import "../../logo/logo.typ": wordmark, mark, mark-box
#import "../mascot.typ": mascot

#let palette = toml("../../palette.toml")
#let formats = toml("../formats.toml")
#let releases = toml("releases.toml").releases

#let name = sys.inputs.at("scheme", default: "light")
#let scheme = palette.at(name)
#let format = formats.at(sys.inputs.at("format", default: "github"))
#let release = releases.find(r => r.version == sys.inputs.at("version", default: releases.first().version))

// Every length is a share of the shorter side, so the cards agree at every size.
#let side = calc.min(format.width, format.height) * 1pt
#let margin = side * 0.12
#let wide = format.width > 1.5 * format.height

#set page(width: format.width * 1pt, height: format.height * 1pt, margin: margin, fill: rgb(scheme.background))
#set text(font: "JetBrains Mono", fill: rgb(scheme.text))

// The v, as tall as the shorter side and cut by the right and bottom edges, as on the social cards.
#let echo = side / mark-box.height
#place(bottom + right, dx: margin + mark-box.width * echo * 0.3, dy: margin + mark-box.height * echo * 0.12,
  box(width: mark-box.width * echo, height: mark-box.height * echo,
    move(dx: -mark-box.left * echo, dy: -mark-box.top * echo,
      mark(echo, scheme.echo))))

#let heading = stack(
  spacing: side * 0.05,
  move(dx: -mark-box.left * side * 0.12 / 200, wordmark(side * 0.12, accent: scheme.accent, ink: scheme.ink)),
  text(size: side * 0.13, weight: "bold", fill: rgb(scheme.ink), release.version),
  text(size: side * 0.036, fill: rgb(scheme.muted), release.date),
)
// The highlights, sized for the longest and its mark to fit the width they are given.
#let highlights(width) = context {
  let line(size, body) = grid(
    columns: (size * 1.4, auto),
    text(size: size, fill: rgb(scheme.accent), "+"),
    text(size: size, body),
  )
  let most = side * 0.045
  let longest = calc.max(..release.highlights.map(h => measure(line(most, h)).width / 1pt)) * 1pt
  let size = most * calc.min(1, width / longest)
  stack(spacing: size * 0.9, ..release.highlights.map(h => line(size, h)))
}

#let inner = format.width * 1pt - 2 * margin
#let gutter = side * 0.1
#place(horizon + left, if wide {
  context {
    let rest = inner - measure(heading).width - gutter
    grid(columns: (auto, 1fr), column-gutter: gutter, align: horizon, heading, highlights(rest))
  }
} else {
  stack(spacing: side * 0.09, heading, highlights(inner))
})
// The mascot on the bottom margin, as on the wide social cards.
#place(bottom + right, dy: margin * 0.5, mascot(name, "glee", side * 0.2))

#let variants = palette.keys().map(s => formats.keys().map(f => releases.map(r =>
  (path: (r.version, s, f), inputs: (scheme: s, format: f, version: r.version)))))
#metadata(variants.flatten()) <variants>
