import '@fontsource-variable/jetbrains-mono';
import 'virtual:brand.css';
import '../style.css';
import './play.css';

import examples from 'virtual:examples';
import build from 'virtual:toolchain';

import {chrome} from '../chrome';
import {Editor} from './editor';
import {serve} from './lsp';
import * as cache from './cache';
import type {Asked, Told, Toolchain} from './protocol';
import {Report} from './report';
import {decode, encode} from './share';

// Where the source and the choice of keys are kept between visits.
const KEPT = 'vun.play.source';
const HELIX = 'vun.play.helix';

const find = <T extends HTMLElement>(selector: string) => document.querySelector(selector) as T;
const status = find<HTMLElement>('[data-status]');
const run = find<HTMLButtonElement>('[data-run]');
const label = find<HTMLElement>('[data-run-label]');

// Storage may be refused, as in a private window: then nothing outlives the page.
const kept = (key: string) => {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
};
const keep = (key: string, value: string) => {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Kept as long as the page.
  }
};

// A shared link's code, else what was last written, else the first example.
const first = () => decode(location.hash.slice(1)) ?? kept(KEPT) ?? examples[0].code;

const editor = new Editor(find('[data-editor]'), first(), (code) => keep(KEPT, code), () => void start());

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

const tabs = examples.map(({name, code}) => {
  const tab = Object.assign(document.createElement('button'), {type: 'button', textContent: name});
  tab.setAttribute('role', 'tab');
  tab.addEventListener('click', () => {
    editor.code = code;
    editor.view.focus();
  });
  return tab;
});
find('[data-examples]').append(...tabs);

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
const base = import.meta.env.BASE_URL;
const loaded: Promise<Toolchain> = (async () => {
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
  report ??= new Report(find('[data-output]'), editor, toolchain.library);
  running = true;
  label.textContent = 'stop';
  run.classList.add('running');
  const code = editor.code;
  report.start(code);
  if (worker === undefined) {
    worker = new Worker(new URL('./build.ts', import.meta.url), {type: 'module'});
    worker.postMessage({kind: 'setup', toolchain} satisfies Asked);
  }
  let since = performance.now();
  let built = '';
  const seconds = () => `${((performance.now() - since) / 1000).toFixed(2)} s`;
  worker.onmessage = ({data}: MessageEvent<Told>) => {
    if (data.kind === 'saved') {
      // A worker started after this one stops begins from them too.
      toolchain.saved = data.files;
      void cache.keep(toolchain.id, data.files);
    } else if (data.kind === 'record') report?.record(data.record);
    else if (data.kind === 'output') report?.output(data.text);
    else if (data.kind === 'started') {
      since = performance.now();
      say(data.stage === 'build' ? 'building' : 'running');
    } else if (data.kind === 'ended') {
      if (data.stage === 'build') say((built = data.code === 0 ? `built in ${seconds()}` : 'the build failed'));
      else say(`${built}, exited with ${data.code}`);
      if (data.stage === 'run' || data.code !== 0) settle();
    } else {
      say(data.stage === 'build' ? `the compiler crashed: ${data.why}` : `the program trapped: ${data.why}`);
      settle();
    }
  };
  worker.postMessage({kind: 'run', code} satisfies Asked);
}

run.addEventListener('click', () => void start());

chrome();
loaded.then(
  (toolchain) => {
    run.disabled = false;
    say('ready');
    // Without shared memory the editor goes on without a server.
    const served = serve(toolchain, editor.code);
    if (served === undefined) return;
    editor.connect(served.transport);
    void served.ended.then((why) => say(`the language server stopped: ${why}`));
  },
  (error) => say(`the compiler did not load: ${error}`),
);
