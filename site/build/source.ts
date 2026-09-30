import {readFileSync, readdirSync} from 'node:fs';
import {fileURLToPath} from 'node:url';

export const REPOSITORY_DIR = fileURLToPath(new URL('../..', import.meta.url));

// The text of the repository's file at `path`.
export function source(path: string): string {
  return readFileSync(`${REPOSITORY_DIR}${path}`, 'utf8');
}

// The bytes of the repository's file at `path`.
export function bytes(path: string): Buffer {
  return readFileSync(`${REPOSITORY_DIR}${path}`);
}

// Every file below the repository's directory `dir`, by its path there.
export function files(dir: string): string[] {
  const entries = readdirSync(`${REPOSITORY_DIR}${dir}`, {recursive: true, withFileTypes: true});
  const root = `${REPOSITORY_DIR}${dir}`;
  return entries.filter((e) => e.isFile()).map((e) => `${e.parentPath}/${e.name}`.slice(root.length).replace(/^\/+/, '')).sort();
}
