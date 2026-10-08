// The benchmark charts of `assets/diagrams/chart.typ` as cards, in every scheme of `palette.toml`.
//
// Compiled by `cargo xtask typeset`, once per variant declared at the end.

#import "/assets/diagrams/diagram.typ": card, theme
#import "/assets/diagrams/chart.typ" as chart

#let charts = (
  run: (title: "Run time of the same programs, built by each compiler", draw: chart.run),
  compile: (title: "Time to build them", draw: chart.compile),
)

#let t = theme(sys.inputs.at("scheme", default: "light"))
#let shown = charts.at(sys.inputs.at("chart", default: "run"))
#card(t, shown.title, (shown.draw)(t))

#let schemes = ("light", "dark")
#metadata(charts.keys().map(c => schemes.map(s => (path: (c, s), inputs: (scheme: s, chart: c)))).flatten()) <variants>
