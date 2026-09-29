// What the scripts share: the schemes of `palette.toml` and writing a directory of `assets`.

import {mkdirSync, readFileSync, rmSync, writeFileSync} from 'node:fs';

import {parse} from 'smol-toml';

import type {Scheme} from '../src/brand/palette.ts';

// The `assets` directory.
export const ASSETS = new URL('../../', import.meta.url);

export const SCHEMES = parse(readFileSync(new URL('palette.toml', ASSETS), 'utf8')) as unknown as Record<string, Scheme>;

// A directory of `assets` a script writes whole: cleared when opened, so nothing stale stays.
export class Output {
  private readonly root: URL;

  constructor(directory: string) {
    this.root = new URL(`${directory}/`, ASSETS);
    rmSync(this.root, {recursive: true, force: true});
  }

  write(path: string, content: string | Uint8Array) {
    const file = new URL(path, this.root);
    mkdirSync(new URL('.', file), {recursive: true});
    writeFileSync(file, content);
  }
}
