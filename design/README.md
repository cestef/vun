# Design

Vun's visual identity: palette, type, mascot and logo. Every color is written once, in
`tokens/palette.toml`; everything else names it.

```
tokens/      palette.toml (color ramps), type.toml (faces and scale)
themes/      light.toml, dark.toml: roles (art, ui, diag, code) naming ramp steps
mascot/      the hermit crab: parts/<slot>/<variant>.svg, poses/<name>.toml
identity/    logo parts and figures, the code specimen, the identity guide
explore/     candidates under review; see explore/README.md
tools/       build.py and its modules (Python 3.11+, stdlib only)
out/         generated, never edited by hand
```

```sh
python3 design/tools/build.py           # write out/, refresh the part previews
python3 design/tools/build.py --check   # fail if out/ or a part preview is stale
python3 design/tools/build.py --png     # also rasterize every figure (ImageMagick)
```

Outputs: `out/<figure>/<theme>.svg` for every figure in every theme (colors inlined, no CSS, so
they render anywhere), `out/tokens/tokens.{css,typ,json}`, `out/identity.html` (the adopted
identity), `out/explore.html` (the exploration gallery) and `out/tweak.html` (the live tweaker).

## Tweaker

`out/tweak.html` holds every part, preset and wordmark. Pick a preset (colorway or direction),
swap parts per slot, shift hue, saturation, lightness and pastel over all colors at once, or set
each role. The URL hash holds the whole state, so a copied link reproduces the view. The page
exports a colorway TOML (drop it in `explore/mascot/colorways/`) and a pose TOML (drop it in
`mascot/poses/`). Color pickers show the preset color; the hex beside each is the adjusted result
that gets exported.

## Model

- A **part** is an SVG on a shared canvas (512 by 512 for the mascot). Shapes carry role classes:
  `f-<role>` fills, `s-<role>` strokes. The build rewrites each part's `<style id="theme">` with
  the light theme so parts look right in Inkscape, Figma or a browser; edit the shapes, not that
  block.
- A **figure** is a TOML file with `layers`: parts (or other figures) stacked bottom to top, each
  optionally transformed, and a `view` box. Any `*.toml` with `layers` outside `explore/` is
  built in every theme.
- A **theme** maps roles to `ramp.step`. Adding a theme file adds a column to every output.
- A **slot** is a part's directory (`eyes`, `claws`). Explorations swap slots by name.
