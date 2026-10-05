// The workbench both pages are made of: the editor, the panel under it with the problems and what
// the program wrote, the run button and the status line. A page gives it the elements, a first
// text, and what it wants to know when a run ends.

import type {Keys} from '../keys';
import {Dock, type Tab} from './dock';
import {Editor} from './editor';
import type {Outcome} from './lessons';
import {serve} from './lsp';
import {Output} from './output';
import {Problems} from './problems';
import type {Toolchain} from './protocol';
import {Runner} from './runner';
import {keep, kept} from './store';

// Where the choice of keys is kept between visits.
const HELIX = 'vun.play.helix';

const find = <T extends HTMLElement>(root: HTMLElement, name: string) => root.querySelector(`[data-${name}]`) as T;

// How the status line says things are: nothing happening, a run going, or how one ended.
export type State = 'idle' | 'busy' | 'ok' | 'failed';

export interface Options {
  code: string;
  // The text changed.
  changed: (code: string) => void;
  // A run ended as `outcome` says, of the `code` it was started on.
  ended: (outcome: Outcome, code: string) => void;
}

export class Workspace {
  readonly editor: Editor;
  readonly dock: Dock;
  private problems!: Problems;
  private readonly output: Output;
  private readonly tabs: Record<'problems' | 'output', Tab>;
  private readonly status: HTMLElement;
  private readonly dot: HTMLElement;
  private readonly run: HTMLButtonElement;
  private readonly label: HTMLElement;
  private runner: Runner | undefined;
  private ready = false;

  constructor(readonly root: HTMLElement, loaded: Promise<Toolchain>, keys: Keys, private readonly options: Options) {
    this.status = find(root, 'status');
    this.dot = find(root, 'dot');
    this.run = find(root, 'run');
    this.label = find(root, 'run-label');
    this.dock = new Dock(find(root, 'dock'));
    const problemsView = document.createElement('div');
    problemsView.className = 'problems';
    const outputView = document.createElement('div');
    outputView.className = 'report';
    this.tabs = {problems: this.dock.tab('problems', 'Problems', problemsView, 'alert'), output: this.dock.tab('output', 'Output', outputView, 'terminal')};
    this.output = new Output(outputView);

    const pointing = {over: (problem: number | undefined) => this.problems.over(problem), pick: (problem: number) => this.problems.pick(problem)};
    this.editor = new Editor(find(root, 'editor'), options.code, options.changed, () => void this.start(), pointing);
    this.problems = new Problems(problemsView, this.editor, (errors, warnings) => this.counted(errors, warnings), () => this.dock.show('problems'));
    this.problems.start(options.code);
    this.bind(keys);
    void loaded.then(
      (toolchain) => {
        this.runner = new Runner(toolchain);
        this.problems.use(toolchain.library);
        this.run.disabled = false;
        this.ready = true;
        this.say('ready');
        // Without shared memory the editor goes on without a server.
        const served = serve(toolchain, this.editor.code);
        if (served === undefined) return;
        this.editor.connect(served.transport);
        void served.ended.then((why) => this.say(`the language server stopped: ${why}`, 'failed'));
      },
      (error) => this.say(`the compiler did not load: ${error}`, 'failed'),
    );
  }

  say(text: string, state: State = 'idle') {
    this.status.textContent = text;
    this.dot.dataset.state = state;
  }

  // Puts `code` in the editor and clears what the last run showed.
  load(code: string) {
    if (this.runner?.running) this.runner.stop();
    this.settle();
    this.editor.code = code;
    this.problems.start(code);
    this.output.clear();
    this.tabs.problems.mark('');
    this.say(this.ready ? 'ready' : 'loading the compiler');
  }

  // The keys and buttons of the workbench.
  private bind(keys: Keys) {
    const helix = find<HTMLButtonElement>(this.root, 'helix');
    const modal = (on: boolean) => {
      this.editor.helix(on);
      helix.setAttribute('aria-pressed', String(on));
      keep(HELIX, String(on));
    };
    modal(kept(HELIX) === 'true');
    keys.on('helix', () => {
      modal(helix.getAttribute('aria-pressed') !== 'true');
      this.editor.view.focus();
    });
    keys.on('run', () => void this.start());
    keys.on('fold', () => this.dock.toggle());
    keys.on('problems', () => this.dock.show('problems'));
    keys.on('output', () => this.dock.show('output'));
    keys.on('next-problem', () => this.editor.step(1));
    keys.on('previous-problem', () => this.editor.step(-1));
    keys.on('focus-editor', () => this.editor.view.focus());
  }

  private counted(errors: number, warnings: number) {
    this.tabs.problems.mark(errors + warnings === 0 ? '' : String(errors + warnings), errors > 0 ? 'error' : 'warning');
  }

  private settle() {
    this.label.textContent = 'Run';
    this.run.classList.remove('running');
  }

  // Runs the editor's code, or ends the run in progress.
  async start() {
    const {runner} = this;
    if (runner === undefined) return;
    if (runner.running) {
      runner.stop();
      this.settle();
      this.say('stopped');
      return;
    }
    const code = this.editor.code;
    this.label.textContent = 'Stop';
    this.run.classList.add('running');
    this.problems.start(code);
    this.output.clear();
    let since = performance.now();
    let built = '';
    const seconds = () => `${((performance.now() - since) / 1000).toFixed(2)} s`;
    const outcome = await runner.run(code, {
      record: (record) => {
        if (record.type === 'diagnostic') this.problems.add(record);
      },
      output: (text) => {
        this.output.write(text);
        this.dock.show('output');
      },
      started: (stage) => {
        since = performance.now();
        this.say(stage === 'build' ? 'building' : 'running', 'busy');
      },
      ended: (stage, status) => {
        if (stage === 'build') this.say((built = status === 0 ? `built in ${seconds()}` : 'the build failed'), status === 0 ? 'busy' : 'failed');
        else this.say(`${built}, exited with ${status}`, status === 0 ? 'ok' : 'failed');
      },
      crashed: (stage, why) => this.say(stage === 'build' ? `the compiler crashed: ${why}` : `the program trapped: ${why}`, 'failed'),
    });
    if (!runner.running && this.run.classList.contains('running') === false) return;
    this.settle();
    this.problems.finish();
    // What to look at: the problems if there are any, else what the program wrote.
    if (outcome.diagnostics.length > 0) this.dock.show('problems');
    else if (!this.output.empty) this.dock.show('output');
    this.options.ended(outcome, code);
  }
}
