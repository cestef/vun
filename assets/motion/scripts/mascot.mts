// Writes the mascot's files into `assets/mascot`, from its declaration in `src/mascot`:
// `<mood>.svg`, a still of each mood that follows `prefers-color-scheme`; `<scheme>/<mood>.svg`,
// one per scheme of `palette.toml`, for the cards; `motion/<name>.svg`, its CSS animations; and
// `mascot.toml`, what the cards need to know of them.

import {mkdirSync, readFileSync, rmSync, writeFileSync} from 'node:fs';

import {parse} from 'smol-toml';

import type {Scheme} from '../src/brand/palette.ts';
import {MOODS, type MoodName} from '../src/mascot/moods.ts';
import {document, extent, mascot, paint} from '../src/mascot/svg.ts';
import {MOTIONS, REDUCED} from './motions.mts';

const root = new URL('../../', import.meta.url);
const schemes = parse(readFileSync(new URL('palette.toml', root), 'utf8')) as unknown as Record<string, Scheme>;
const out = new URL('mascot/', root);

// Painted in the light scheme, or the dark one when the reader prefers it.
const adaptive = `${paint(schemes.light)}
    @media (prefers-color-scheme: dark) {
      ${paint(schemes.dark)}
    }`;

function write(path: string, text: string) {
  const file = new URL(path, out);
  mkdirSync(new URL('.', file), {recursive: true});
  writeFileSync(file, text);
}

rmSync(out, {recursive: true, force: true});
const box = extent();
const moods = Object.keys(MOODS) as MoodName[];
for (const mood of moods) {
  const drawn = mascot(MOODS[mood].face);
  write(`${mood}.svg`, document(box, adaptive, drawn));
  for (const [name, scheme] of Object.entries(schemes)) {
    write(`${name}/${mood}.svg`, document(box, paint(scheme), drawn));
  }
}
// Room either side of an animation's frame, a share of the body's width, for it to squash and
// shake in.
const SIDES = 0.1;
for (const [name, motion] of Object.entries(MOTIONS)) {
  const above = box.height * motion.headroom;
  const side = box.width * SIDES;
  const framed = {x: box.x - side, y: box.y - above, width: box.width + 2 * side, height: box.height + above};
  const drawn = mascot(...motion.moods.map((mood) => MOODS[mood].face));
  write(`motion/${name}.svg`, document(framed, `${adaptive}\n    ${motion.css}\n    ${REDUCED}`, drawn));
}
write('mascot.toml', `# Written by \`pnpm mascot\` in assets/motion; the cards read it.\n\n# The moods, in order.\nmoods = [${moods.map((m) => `"${m}"`).join(', ')}]\n# The stills' width over their height.\naspect = ${box.width / box.height}\n`);
