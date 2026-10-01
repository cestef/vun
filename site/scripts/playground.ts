// The playground's build and run, as the build worker does them, for a script: the compiler (the
// WASI build of `target/wasm32-wasip1/dist/vun.wasm`) run in a filesystem held in memory, then the
// program it wrote. Whatever the page would show is returned.

import {File, PreopenDirectory} from '@bjorn3/browser_wasi_shim';

import type {Outcome} from '../src/play/lessons.ts';
import type {Diagnostic, Reported, Toolchain} from '../src/play/protocol.ts';
import {ENV, ROOT, command, executable, filesystem, writer} from '../src/play/wasi.ts';
import {bytes, files, source} from '../build/source.ts';

const PROGRAM = 'main.wasm';

// The toolchain as the page loads it: the compiler and the library, with no memos saved.
export async function load(): Promise<Toolchain> {
  const executable = new Uint8Array(bytes('target/wasm32-wasip1/dist/vun.wasm'));
  const library = Object.fromEntries(files('lib').map((path) => [path, source(`lib/${path}`)]));
  return {id: 'check', compiler: await WebAssembly.compile(executable), executable, library, saved: []};
}

// Builds and runs `code`, a fresh filesystem each time, so that no run reads another's memos.
export async function play(toolchain: Toolchain, code: string): Promise<Outcome> {
  const root = filesystem(toolchain, code);
  const diagnostics: Diagnostic[] = [];
  let line = '';
  let text = '';
  const records = writer((chunk) => {
    const lines = (line + chunk).split('\n');
    line = lines.pop() as string;
    for (const whole of lines) {
      try {
        const record: Reported = JSON.parse(whole);
        if (record.type === 'diagnostic') diagnostics.push(record);
      } catch {
        text += `${whole}\n`;
      }
    }
  });
  const args = [executable(toolchain), 'build', ROOT, '-o', PROGRAM, '--format', 'json'];
  const built = await command(toolchain.compiler, args, ENV, new PreopenDirectory('/', root), records);
  const program = root.get(PROGRAM);
  const outcome: Outcome = {built: 'code' in built && built.code === 0 && program instanceof File, diagnostics, output: '', exit: undefined, trap: 'trap' in built ? built.trap : undefined, notes: text};
  if (!outcome.built || !(program instanceof File)) return outcome;
  const output = writer((chunk) => void (outcome.output += chunk));
  const ran = await command(await WebAssembly.compile(program.data as BufferSource), ['main'], [], new PreopenDirectory('.', new Map()), output);
  if ('code' in ran) outcome.exit = ran.code;
  else outcome.trap = ran.trap;
  return outcome;
}
