// Records what `src/episodes/<episode>/record.toml` asks for, beside it. `session.json`: each
// command of its session run with `--format styled`, both streams into one file so their lines
// keep their order, each line as its pieces; a line that is not one of vun's is the program's own
// output, kept as plain text. `VUN` names the command to run as vun (words, as `PAGER` does),
// `vun` unless given. `changes.json`: each change's file before and after it, from `jj`.

import {closeSync, mkdtempSync, openSync, readFileSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawnSync} from 'node:child_process';

import {parse} from 'smol-toml';

import type {Line, Piece} from '../src/clip/episode.ts';
import {ASSETS} from './assets.mts';

// The program the commands are shown running.
const PROGRAM = 'vun';

interface Asked {
  session?: {directory: string; commands: string[][]};
  changes?: {change: string; file: string}[];
}

const episode = process.argv[2];
if (episode === undefined) {
  throw new Error('record which episode? `pnpm record <episode>`, one of src/episodes');
}
const folder = new URL(`../src/episodes/${episode}/`, import.meta.url);
const asked = parse(readFileSync(new URL('record.toml', folder), 'utf8')) as unknown as Asked;
const [runner, ...words] = (process.env.VUN ?? PROGRAM).split(' ').filter((w) => w !== '');
const repository = new URL('../', ASSETS);
const scratch = mkdtempSync(join(tmpdir(), 'vun-record-'));
const write = (file: string, value: unknown) => writeFileSync(new URL(file, folder), `${JSON.stringify(value, null, 2)}\n`);

// What `words` print, run in the repository.
function output(program: string, args: string[]): string {
  const run = spawnSync(program, args, {cwd: repository, encoding: 'utf8'});
  if (run.error !== undefined || run.status !== 0) {
    throw new Error(`\`${[program, ...args].join(' ')}\` failed: ${run.stderr}`, {cause: run.error});
  }
  return run.stdout;
}

// The line `text` wrote: one of vun's, or the program's own.
function line(text: string): Line {
  try {
    const record = JSON.parse(text) as {type?: unknown; pieces?: Piece[]};
    if (record.type === 'line' && Array.isArray(record.pieces)) {
      return {pieces: record.pieces};
    }
  } catch {
    // Not JSON: the program's own output.
  }
  return {pieces: text === '' ? [] : [{text}]};
}

if (asked.session !== undefined) {
  const {directory, commands} = asked.session;
  const cwd = new URL(directory, repository);
  const steps = commands.map((args, i) => {
    const file = join(scratch, `${i}.out`);
    const fd = openSync(file, 'w');
    const run = spawnSync(runner, [...words, '--format', 'styled', ...args], {cwd, stdio: ['ignore', fd, fd]});
    closeSync(fd);
    if (run.error !== undefined) {
      throw new Error(`could not run ${runner}`, {cause: run.error});
    }
    const printed = readFileSync(file, 'utf8').replace(/\n$/, '').split('\n');
    return {command: [PROGRAM, ...args], output: printed.map(line)};
  });
  write('session.json', {directory, steps});
}

if (asked.changes !== undefined) {
  // The file at a revision, as jj shows it.
  const at = (revision: string, file: string) => output('jj', ['file', 'show', '--ignore-working-copy', '-r', revision, file]);
  write(
    'changes.json',
    asked.changes.map(({change, file}) => ({change, file, before: at(`${change}-`, file), after: at(change, file)})),
  );
}
