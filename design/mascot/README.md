# Mascot

A purple pincher hermit crab (*Coenobita clypeatus*) in a faceted stone, after Dwebble; flat
color, no outlines, in the manner of Ferris and the Go gopher.

## Anatomy

Every part is drawn on the same 512 by 512 canvas, so any variant of a slot fits any pose.

| slot     | variants            | anchors                                            |
|----------|---------------------|----------------------------------------------------|
| `shell`  | stone               | hull within x 90..440, y 40..350; hollow at 256,312 |
| `legs`   | stand               | two pairs from under the body                      |
| `body`   | base                | ellipse at 256,326                                 |
| `stalks` | up                  | from the body to the eyes                          |
| `eyes`   | open, happy, closed | centered at 214,184 and 298,184                    |
| `cheeks` | blush               | 220,318 and 292,318                                |
| `mouth`  | smile, open         | 256,322                                            |
| `claws`  | rest, raised        | the big claw on the viewer's left, as on the crab  |

Layer order, bottom to top: shell, legs, body, stalks, eyes, cheeks, mouth, claws.

The claw is one path in local space (palm at the origin, pincer up), placed by each arm's
`transform`; keep its `d` identical everywhere it appears.

## Roles

`shell-top`, `shell-hi`, `shell`, `shell-lo`, `shell-deep` (facets, lit from the top left),
`hollow`, `fleck`, `body`, `body-lo`, `ink`, `glint`, `mouth`, `cheek`.

## Adding

- A variant: draw `parts/<slot>/<name>.svg` on the canvas with role classes, run the build.
- A pose: `poses/<name>.toml` listing the layers.
- A role: add it to every file in `themes/`; the build names a missing one.
