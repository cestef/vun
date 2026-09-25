# Exploration

Candidates for the mascot and identity, rendered into `out/explore.html` with an id on each for
feedback (`S3`, `C4`, `D2`, ...). Nothing here is built into the adopted outputs; once a choice
is made, its parts move to `mascot/` or `identity/` and its colors to `tokens/` and `themes/`.

```
mascot/parts/<slot>/   extra part variants
mascot/variants.toml   gallery groups: slot swaps on a base pose, with a colorway
mascot/colorways/      art-role overrides over the light theme, optionally with own ramps
identity/directions/   whole directions: ramps, ui/diag/code/art roles, type, mascot swaps
identity/wordmarks/    drawn wordmarks
identity/marks/        logo marks and lockups (figures)
identity/type.toml     display faces for the name
```

A colorway or direction may declare `[ramps.<name>]` for colors not in the palette yet, `base`
(the theme it overlays, default `light`), `backdrop` and `order`.
