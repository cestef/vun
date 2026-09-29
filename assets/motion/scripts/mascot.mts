// Writes the mascot's files into `assets/mascot`, from its declaration in `src/mascot`:
// `<mood>.svg`, a still of each mood that follows `prefers-color-scheme`; `<scheme>/<mood>.svg`,
// one per scheme of `palette.toml`, for the cards; `motion/<name>.svg`, its CSS animations; and
// `mascot.toml`, what the cards need to know of them.

import {MOODS, type MoodName} from '../src/mascot/moods.ts';
import {MASCOT, radii} from '../src/mascot/shape.ts';
import {document, extent, mascot, paint} from '../src/mascot/svg.ts';
import {Output, SCHEMES} from './assets.mts';
import {MOTIONS, animated} from './motions.mts';

// Painted in the light scheme, or the dark one when the reader prefers it.
const adaptive = `${paint(SCHEMES.light)}
    @media (prefers-color-scheme: dark) {
      ${paint(SCHEMES.dark)}
    }`;

const out = new Output('mascot');
const box = extent();
const moods = Object.keys(MOODS) as MoodName[];
for (const mood of moods) {
  const drawn = mascot(MOODS[mood].face);
  out.write(`${mood}.svg`, document(box, adaptive, drawn));
  for (const [name, scheme] of Object.entries(SCHEMES)) {
    out.write(`${name}/${mood}.svg`, document(box, paint(scheme), drawn));
  }
}
for (const [name, motion] of Object.entries(MOTIONS)) {
  out.write(`motion/${name}.svg`, animated(motion, adaptive));
}
// The outline's corners round the body's by half its weight.
const corners = radii(MASCOT.body).map((r) => (r + MASCOT.weight / 2) / box.height);
out.write(
  'mascot.toml',
  `# Written by \`pnpm mascot\` in assets/motion; the cards read it.

# The moods, in order.
moods = [${moods.map((m) => `"${m}"`).join(', ')}]
# The stills' width over their height.
aspect = ${box.width / box.height}
# The radii of the stills' corners from the top left, clockwise, per unit of their height.
corners = [${corners.join(', ')}]
`,
);
