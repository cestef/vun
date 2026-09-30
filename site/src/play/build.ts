// The build worker: the compiler builds the root file for WASI, its records passed on as they
// come, then the program it wrote runs. Its filesystem lives as long as the worker, so each build
// starts from the memos the last one saved.

import {File, type Inode, PreopenDirectory} from '@bjorn3/browser_wasi_shim';

import type {Asked, Stage, Told, Toolchain} from './protocol';
import {ENV, ROOT, command, executable, filesystem, saved, writer} from './wasi';

const PROGRAM = 'main.wasm';

const tell = (told: Told) => postMessage(told);

// The compiler's output as records, a line each; a line that is not one is passed on as text.
function records() {
  let line = '';
  return writer((text) => {
    const lines = (line + text).split('\n');
    line = lines.pop() as string;
    for (const whole of lines) {
      try {
        tell({kind: 'record', record: JSON.parse(whole)});
      } catch {
        if (whole.trim() !== '') tell({kind: 'output', stage: 'build', text: `${whole}\n`});
      }
    }
  });
}

async function run(stage: Stage, ran: Promise<{code: number} | {trap: string}>) {
  const ended = await ran;
  if ('code' in ended) tell({kind: 'ended', stage, code: ended.code});
  else tell({kind: 'crashed', stage, why: ended.trap});
  return 'code' in ended ? ended.code : undefined;
}

let toolchain: Toolchain;
let root: Map<string, Inode>;

async function build(code: string) {
  root.set(ROOT, new File(new TextEncoder().encode(code)));
  root.delete(PROGRAM);
  tell({kind: 'started', stage: 'build'});
  const args = [executable(toolchain), 'build', ROOT, '-o', PROGRAM, '--format', 'json'];
  const built = await run('build', command(toolchain.compiler, args, ENV, new PreopenDirectory('/', root), records()));
  tell({kind: 'saved', files: saved(root)});
  const program = root.get(PROGRAM);
  if (built !== 0 || !(program instanceof File)) return;
  tell({kind: 'started', stage: 'run'});
  const output = writer((text) => tell({kind: 'output', stage: 'run', text}));
  // Relative paths are the program's own directory's, which a host names `.`.
  const dir = new PreopenDirectory('.', new Map());
  await run('run', command(await WebAssembly.compile(program.data as BufferSource), ['main'], [], dir, output));
}

onmessage = ({data}: MessageEvent<Asked>) => {
  if (data.kind === 'setup') {
    toolchain = data.toolchain;
    root = filesystem(toolchain, '');
  } else void build(data.code);
};
