// What both workers share: the compiler run as a WASI command in a filesystem held in memory.

import {ConsoleStdout, Directory, type Fd, File, type Inode, OpenFile, PreopenDirectory, WASI} from '@bjorn3/browser_wasi_shim';

import type {Toolchain} from './protocol';

// The root file, at the root of the compiler's filesystem, and the directory its builds keep
// their files in beside it: the memos saved, and what each build wrote, which is not kept.
export const ROOT = 'main.vun';
const WORK = '.vun';
const BUILDS = 'build';
const TARGET = 'wasi.wasm32.p1';

// What the compiler reads: the target it builds for.
export const ENV = [`VUN_TARGET=${TARGET}`];

// Where the toolchain is laid out as a release unpacks, the executable in `bin/` beside the
// library in `lib/`, in a directory named by its build: the compiler finds its library beside it,
// and tells its memos from another build's by its path.
const home = (toolchain: Toolchain) => `toolchain/${toolchain.id}`;

// The executable's path, as the compiler's first argument names it where WASI names none.
export const executable = (toolchain: Toolchain) => `/${home(toolchain)}/bin/vun`;

const encoder = new TextEncoder();

// A directory holding each of `files` at its path, made of the directories on the way.
function tree(files: [string, Uint8Array][]): Map<string, Inode> {
  const root = new Map<string, Inode>();
  for (const [path, data] of files) {
    const parts = path.split('/');
    const name = parts.pop() as string;
    let dir = root;
    for (const part of parts) {
      let next = dir.get(part);
      if (!(next instanceof Directory)) dir.set(part, (next = new Directory(new Map())));
      dir = (next as Directory).contents;
    }
    dir.set(name, new File(data));
  }
  return root;
}

// The compiler's filesystem: the toolchain, the memos saved before, and `code` as the root file.
export function filesystem(toolchain: Toolchain, code: string): Map<string, Inode> {
  const library = Object.entries(toolchain.library).map(([path, text]): [string, Uint8Array] => [`${home(toolchain)}/lib/${path}`, encoder.encode(text)]);
  const bin: [string, Uint8Array] = [executable(toolchain).slice(1), toolchain.executable];
  return tree([bin, ...library, ...toolchain.saved, [ROOT, encoder.encode(code)]]);
}

// The files the compiler saved in the work directory, by path, but what its builds wrote.
export function saved(root: Map<string, Inode>): [string, Uint8Array][] {
  const files: [string, Uint8Array][] = [];
  const walk = (dir: Map<string, Inode>, at: string) => {
    for (const [name, inode] of dir) {
      if (inode instanceof Directory && `${at}${name}` !== `${WORK}/${BUILDS}`) walk(inode.contents, `${at}${name}/`);
      else if (inode instanceof File) files.push([`${at}${name}`, inode.data]);
    }
  };
  const work = root.get(WORK);
  if (work instanceof Directory) walk(work.contents, `${WORK}/`);
  return files;
}

// Output decoded as it comes, whatever splits it.
export function writer(write: (text: string) => void): ConsoleStdout {
  const decoder = new TextDecoder();
  return new ConsoleStdout((bytes) => write(decoder.decode(bytes, {stream: true})));
}

// Runs `module` as a WASI command with `args`, standard input `input` (empty without), `output`
// as its standard output and error, and the directory `dir`: its exit code, or the trap that
// ended it.
export async function command(
  module: WebAssembly.Module,
  args: string[],
  env: string[],
  dir: PreopenDirectory,
  output: Fd,
  input: Fd = new OpenFile(new File([])),
): Promise<{code: number} | {trap: string}> {
  const wasi = new WASI(args, env, [input, output, output, dir], {debug: false});
  try {
    const instance = await WebAssembly.instantiate(module, {wasi_snapshot_preview1: wasi.wasiImport});
    return {code: wasi.start(instance as never)};
  } catch (error) {
    return {trap: String(error)};
  }
}
