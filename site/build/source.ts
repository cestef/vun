import {readFileSync, readdirSync} from 'node:fs';
import {resolve} from 'node:path';

// The repository's root, the site's parent: the site is built and tested from its own directory.
export const REPOSITORY_DIR = `${resolve(process.cwd(), '..')}/`;

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
  const root = `${REPOSITORY_DIR}${dir}`;
  return readdirSync(root, {recursive: true, withFileTypes: true})
    .filter((entry) => entry.isFile())
    .map((entry) => `${entry.parentPath}/${entry.name}`.slice(root.length).replace(/^\/+/, ''))
    .sort();
}
