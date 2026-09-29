// Writes what the stream overlays of `assets/stream` draw from into `assets/stream/generated`:
// `schemes.js`, the schemes of `palette.toml` for `overlay.js` to paint with, and
// `figures/<scheme>/`, the mascot's CSS animations (`mascot/<motion>.svg`) and the logo's
// (`logo/<name>.svg`, from `assets/logo/motion`) painted in each scheme.

import {readFileSync, readdirSync} from 'node:fs';

import {paint} from '../src/mascot/svg.ts';
import type {Scheme} from '../src/brand/palette.ts';
import {ASSETS, Output, SCHEMES} from './assets.mts';
import {MOTIONS, animated} from './motions.mts';

// The role of `palette.toml` each class of the logo's animations is stroked in.
const LOGO: Record<string, keyof Scheme> = {v: 'accent', un: 'ink'};

const logos = new URL('logo/motion/', ASSETS);
const out = new Output('stream/generated');

out.write(
  'schemes.js',
  `// Written by \`pnpm stream\` in assets/motion from palette.toml.\nconst SCHEMES = ${JSON.stringify(SCHEMES, null, 2)};\n`,
);
for (const [name, scheme] of Object.entries(SCHEMES)) {
  for (const [motion, declared] of Object.entries(MOTIONS)) {
    out.write(`figures/${name}/mascot/${motion}.svg`, animated(declared, paint(scheme)));
  }
  // A style sheet after the file's own paints its classes in the scheme, over its adaptive colors.
  const rules = Object.entries(LOGO)
    .map(([part, role]) => `.${part} { stroke: ${scheme[role]}; }`)
    .join(' ');
  for (const file of readdirSync(logos)) {
    const source = readFileSync(new URL(file, logos), 'utf8');
    out.write(`figures/${name}/logo/${file}`, source.replace('</svg>', `<style>${rules}</style>\n</svg>`));
  }
}
