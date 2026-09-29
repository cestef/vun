// The mascot's moods side by side, named, in every scheme of `palette.toml`.
//
// Compiled by `cargo xtask typeset` at 72 pixels per inch, so a point is a pixel, once per
// variant declared at the end.

#import "../mascot.typ": mascot, moods

#let palette = toml("../../palette.toml")
#let name = sys.inputs.at("scheme", default: "light")
#let scheme = palette.at(name)

#let height = 120pt

#set page(width: auto, height: auto, margin: 64pt, fill: rgb(scheme.background))
#set text(font: "JetBrains Mono", size: 18pt, fill: rgb(scheme.muted))

#stack(dir: ltr, spacing: 48pt, ..moods.map(mood => stack(spacing: 22pt,
  mascot(name, mood, height),
  align(center, text(mood)))))

#metadata(palette.keys().map(s => (path: (s,), inputs: (scheme: s)))) <variants>
