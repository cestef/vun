// A code card for the README: a corpus program, highlighted, on a rounded panel; under `mascot/`,
// the mascot sitting on the panel.
//
// Compiled by `cargo xtask typeset`, once per variant declared at the end.

#import "../mascot.typ": mascot

#let source = "/corpus/accept/programs/tour.vun"
#let scheme = sys.inputs.at("scheme", default: "light")
#let colors = toml("colors.toml").at(scheme)
// The mascot's mood, or none.
#let mood = sys.inputs.at("mascot", default: none)

#let entry(key, value) = "<key>" + key + "</key><string>" + value + "</string>"
#let rule(scope, role, style) = (
  "<dict><key>scope</key><string>" + scope + "</string><key>settings</key><dict>"
    + entry("foreground", colors.at(role)) + entry("fontStyle", style) + "</dict></dict>"
)
#let theme = bytes(
  "<?xml version=\"1.0\" encoding=\"UTF-8\"?><plist version=\"1.0\"><dict>"
    + entry("name", "Vun " + scheme) + "<key>settings</key><array>"
    + "<dict><key>settings</key><dict>" + entry("foreground", colors.text) + "</dict></dict>"
    + toml("roles.toml").roles.map(r => rule(r.scopes.join(", "), r.color, r.style)).join()
    + "</array></dict></plist>"
)

// The program without its module doc, which says why it is in the corpus.
#let code = read(source).split("\n").filter(l => not l.starts-with("//!")).join("\n").trim()

#set page(width: auto, height: auto, margin: 0pt, fill: none)
#set text(font: "JetBrains Mono", size: 10pt, fill: rgb(colors.text))
#show raw: set text(font: "JetBrains Mono", ligatures: true, features: ("calt",))

#let dot = circle(radius: 3.2pt, fill: rgb(colors.border))
#let bar = stack(dir: ltr, spacing: 5pt, dot, dot, dot, h(10pt),
  text(size: 8.5pt, fill: rgb(colors.comment), baseline: -0.5pt, source.split("/").last()))

#let panel = block(
  fill: rgb(colors.background),
  stroke: 0.75pt + rgb(colors.border),
  radius: 10pt,
  clip: true,
  grid(
    fill: (rgb(colors.bar), none),
    grid.cell(inset: (x: 14pt, y: 9pt), bar),
    grid.hline(stroke: 0.75pt + rgb(colors.border)),
    grid.cell(inset: (x: 18pt, top: 14pt, bottom: 18pt),
      raw(code, lang: "vun", block: true, syntaxes: "/editors/sublime/vun.sublime-syntax", theme: theme)),
  ),
)

// The mascot sits on the panel's top right, its base hidden behind the panel's edge.
#let sitting = 4pt
#if mood == none { panel } else {
  stack(spacing: -sitting, align(right, pad(right: 36pt, mascot(scheme, mood, 52pt))), panel)
}

#let schemes = toml("colors.toml").keys()
#let plain = schemes.map(s => (path: (s,), inputs: (scheme: s)))
#let mascots = schemes.map(s => (path: ("mascot", s), inputs: (scheme: s, mascot: "glee")))
#metadata(plain + mascots) <variants>
