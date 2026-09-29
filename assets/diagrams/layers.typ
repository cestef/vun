// The layering of the compiler's crates, from `layers.toml`, which `cargo xtask layers` writes.

#import "diagram.typ": ranks

#let diagram(t) = ranks(t, toml("layers.toml").ranks, rule: "may depend on the ranks below")
