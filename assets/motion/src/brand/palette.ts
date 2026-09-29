// The brand's colors: the schemes of `palette.toml`, which the cards read too.

import {useScene} from '@motion-canvas/core';
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

// The entry `name` of `table`, or an error naming the `kind` asked for and those there are.
export function entry<T>(table: Partial<Record<string, T>>, name: string, kind: string): T {
  const found = table[name];
  if (found === undefined) {
    throw new Error(`no ${kind} ${name}; there are ${Object.keys(table).join(', ')}`);
  }
  return found;
}

export function scheme(name: string): Scheme {
  return entry(SCHEMES, name, 'scheme');
}

// The name of the scheme the scene's variable `scheme` asks for.
export function useSchemeName(): string {
  return useScene().variables.get('scheme', 'dark')();
}

export function useScheme(): Scheme {
  return scheme(useSchemeName());
}
