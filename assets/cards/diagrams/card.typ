// The diagrams of `assets/diagrams` as cards, in every scheme of `palette.toml`.
//
// Compiled by `cargo xtask typeset`, once per variant declared at the end.

#import "/assets/diagrams/diagram.typ": card, theme
#import "/assets/diagrams/pipeline.typ" as pipeline
#import "/assets/diagrams/layers.typ" as layers
#import "/assets/diagrams/origins.typ" as origins

#let diagrams = (
  pipeline: (title: "The compiler's stages", draw: pipeline.diagram),
  layers: (title: "The layers of the compiler's crates", draw: layers.diagram),
  origins: (title: "Every entity points back to its source", draw: origins.diagram),
)

#let t = theme(sys.inputs.at("scheme", default: "light"))
#let shown = diagrams.at(sys.inputs.at("diagram", default: "pipeline"))
#card(t, shown.title, (shown.draw)(t))

#let schemes = toml("/assets/palette.toml").keys()
#metadata(diagrams.keys().map(d => schemes.map(s => (path: (d, s), inputs: (scheme: s, diagram: d)))).flatten()) <variants>
