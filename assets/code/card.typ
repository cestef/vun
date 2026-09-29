// A code card for the README: a corpus program, highlighted, on a rounded panel.
//
//   typst compile --root . --ppi 200 --input scheme=light assets/code/card.typ assets/code-light.png
//   typst compile --root . --ppi 200 --input scheme=dark assets/code/card.typ assets/code-dark.png

#let source = "/corpus/accept/programs/tour.vun"
#let scheme = sys.inputs.at("scheme", default: "light")
#let colors = toml("colors.toml").at(scheme)

// Highlighting scopes of `vun.sublime-syntax`, each drawn in a role of `colors.toml`.
#let roles = (
  ("comment", "comment", "italic"),
  ("string, constant.character", "string", ""),
  ("constant.numeric, constant.language, variable.other.member", "literal", ""),
  ("keyword.control, keyword.other, keyword.operator.stage, keyword.operator.quote", "keyword", ""),
  ("keyword.operator, punctuation", "punct", ""),
  ("entity.name.function, variable.function", "name", "bold"),
)

#let entry(key, value) = "<key>" + key + "</key><string>" + value + "</string>"
#let rule(scope, role, style) = (
  "<dict><key>scope</key><string>" + scope + "</string><key>settings</key><dict>"
    + entry("foreground", colors.at(role)) + entry("fontStyle", style) + "</dict></dict>"
)
#let theme = bytes(
  "<?xml version=\"1.0\" encoding=\"UTF-8\"?><plist version=\"1.0\"><dict>"
    + entry("name", "Vun " + scheme) + "<key>settings</key><array>"
    + "<dict><key>settings</key><dict>" + entry("foreground", colors.text) + "</dict></dict>"
    + roles.map(r => rule(..r)).join()
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

#block(
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
