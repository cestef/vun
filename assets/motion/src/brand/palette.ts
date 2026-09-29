// The brand's colors: the schemes of `palette.toml`, which the cards read too.

import {parse} from 'smol-toml';

import palette from '../../../palette.toml?raw';

export interface Scheme {
  background: string;
  text: string;
  muted: string;
  accent: string;
  ink: string;
  echo: string;
}

export const SCHEMES = parse(palette) as unknown as Partial<Record<string, Scheme>>;

// The scheme `name`, or an error naming those there are.
export function scheme(name: string): Scheme {
  const found = SCHEMES[name];
  if (found === undefined) {
    throw new Error(`no scheme ${name}; there are ${Object.keys(SCHEMES).join(', ')}`);
  }
  return found;
}
