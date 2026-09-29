// The mascot for the cards: its stills, written by `pnpm mascot` in assets/motion from its
// declaration there, one per scheme of `palette.toml` and mood.

#let info = toml("../mascot/mascot.toml")
#let moods = info.moods

// The mascot of the scheme named `scheme` in `mood`, `height` tall.
#let mascot(scheme, mood, height) = image("../mascot/" + scheme + "/" + mood + ".svg", height: height)

// How wide the mascot is when `height` tall.
#let width(height) = height * info.aspect
