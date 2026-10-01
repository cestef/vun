import '@fontsource-variable/jetbrains-mono';
import 'virtual:brand.css';
import '../style.css';
import './play.css';

import lessons from 'virtual:lessons';
import build from 'virtual:toolchain';

import {chrome} from '../chrome';
import {Editor} from './editor';
import {Guide} from './guide';
import {isolate} from './isolate';
import type {Outcome} from './lessons';
import {serve} from './lsp';
import * as cache from './cache';
import type {Asked, Told, Toolchain} from './protocol';
import {Report} from './report';
import {decode, encode} from './share';
import {keep, kept} from './store';

// Where the choice of keys is kept between visits.
const HELIX = 'vun.play.helix';

const base = import.meta.env.BASE_URL;

const find = <T extends HTMLElement>(selector: string) => document.querySelector(selector) as T;
const status = find<HTMLElement>('[data-status]');
const run = find<HTMLButtonElement>('[data-run]');
const label = find<HTMLElement>('[data-run-label]');

const output = find<HTMLElement>('[data-output]');
let ready = false;

// The guide shows a lesson's code in the editor, and the code is kept as the lesson's. A shared
// link's code goes to the last lesson, which is free.
const guide = new Guide(find('[data-guide]'), lessons, (code) => {
  if (running) stop();
  editor.code = code;
  output.replaceChildren();
  say(ready ? 'ready' : 'loading the compiler');
});
const linked = decode(location.hash.slice(1));
if (linked !== undefined) guide.free(linked);

const editor = new Editor(find('[data-editor]'), guide.opening, (code) => guide.changed(code), () => void start());
guide.render();

const helix = find<HTMLButtonElement>('[data-helix]');
const modal = (on: boolean) => {
  editor.helix(on);
  helix.setAttribute('aria-pressed', String(on));
  keep(HELIX, String(on));
};
modal(kept(HELIX) === 'true');
helix.addEventListener('click', () => {
  modal(helix.getAttribute('aria-pressed') !== 'true');
  editor.view.focus();
});

const share = find<HTMLButtonElement>('[data-share]');
let shared = 0;
share.addEventListener('click', async () => {
  const link = `${location.origin}${location.pathname}#${encode(editor.code)}`;
  history.replaceState(null, '', link);
  try {
    await navigator.clipboard.writeText(link);
    share.textContent = 'link copied';
  } catch {
    share.textContent = 'in the address bar';
  }
  clearTimeout(shared);
  shared = window.setTimeout(() => (share.textContent = 'share'), 1600);
});

// The compiler, its library and the memos kept from earlier visits, loaded once; each worker is
// given them.
const loaded: Promise<Toolchain> = (async () => {
  // Before the downloads, which a page about to load again would make twice.
  await isolate(base);
  const executable = new Uint8Array(await (await fetch(`${base}play/vun.wasm`)).arrayBuffer());
  const [compiler, library, saved] = await Promise.all([
    WebAssembly.compile(executable),
    fetch(`${base}play/library.json`).then((r) => r.json() as Promise<Record<string, string>>),
    cache.load(build),
  ]);
  return {id: build, compiler, executable, library, saved};
})();

let worker: Worker | undefined;
let running = false;
let report: Report | undefined;

const say = (text: string) => (status.textContent = text);

function stop() {
  worker?.terminate();
  worker = undefined;
  settle();
  say('stopped');
}

function settle() {
  running = false;
  label.textContent = 'run';
  run.classList.remove('running');
}

async function start() {
  if (running) return stop();
  const toolchain = await loaded;
  report ??= new Report(output, editor, toolchain.library);
  running = true;
  label.textContent = 'stop';
  run.classList.add('running');
  const code = editor.code;
  // The lesson the run is of, whichever the guide shows when it ends.
  const lesson = guide.lesson;
  report.start(code);
  if (worker === undefined) {
    worker = new Worker(new URL('./build.ts', import.meta.url), {type: 'module'});
    worker.postMessage({kind: 'setup', toolchain} satisfies Asked);
  }
  let since = performance.now();
  let built = '';
  // What the run did, for the guide to judge.
  const outcome: Outcome = {built: false, diagnostics: [], output: '', exit: undefined, trap: undefined, notes: ''};
  const seconds = () => `${((performance.now() - since) / 1000).toFixed(2)} s`;
  worker.onmessage = ({data}: MessageEvent<Told>) => {
    if (data.kind === 'saved') {
      // A worker started after this one stops begins from them too.
      toolchain.saved = data.files;
      void cache.keep(toolchain.id, data.files);
    } else if (data.kind === 'record') {
      report?.record(data.record);
      if (data.record.type === 'diagnostic') outcome.diagnostics.push(data.record);
    } else if (data.kind === 'output') {
      report?.output(data.text);
      if (data.stage === 'run') outcome.output += data.text;
      else outcome.notes += data.text;
    } else if (data.kind === 'started') {
      since = performance.now();
      say(data.stage === 'build' ? 'building' : 'running');
    } else if (data.kind === 'ended') {
      if (data.stage === 'build') {
        outcome.built = data.code === 0;
        say((built = outcome.built ? `built in ${seconds()}` : 'the build failed'));
      } else {
        outcome.exit = data.code;
        say(`${built}, exited with ${data.code}`);
      }
      if (data.stage === 'run' || data.code !== 0) {
        settle();
        guide.judged(lesson, outcome);
      }
    } else {
      outcome.trap = data.why;
      say(data.stage === 'build' ? `the compiler crashed: ${data.why}` : `the program trapped: ${data.why}`);
      settle();
      guide.judged(lesson, outcome);
    }
  };
  worker.postMessage({kind: 'run', code} satisfies Asked);
}

run.addEventListener('click', () => void start());

chrome();
loaded.then(
  (toolchain) => {
    run.disabled = false;
    ready = true;
    say('ready');
    // Without shared memory the editor goes on without a server.
    const served = serve(toolchain, editor.code);
    if (served === undefined) return;
    editor.connect(served.transport);
    void served.ended.then((why) => say(`the language server stopped: ${why}`));
  },
  (error) => say(`the compiler did not load: ${error}`),
);
