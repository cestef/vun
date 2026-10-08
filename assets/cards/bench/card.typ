// The benchmark chart of `assets/diagrams/chart.typ` as a card, in light and dark.
//
// Compiled by `cargo xtask typeset`, once per variant declared at the end.

#import "/assets/diagrams/diagram.typ": card, theme
#import "/assets/diagrams/chart.typ" as chart

#let t = theme(sys.inputs.at("scheme", default: "light"))
#card(t, "Run time and build time of the same programs, in each language", chart.both(t), radius: t.unit * 1.5)

#metadata(("light", "dark").map(s => (path: (s,), inputs: (scheme: s)))) <variants>
