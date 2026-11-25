// What the workbench knows: the editor, the last build's problems, what the program wrote, the
// status line and which tab of the panel is open. The views read it, and the keys and buttons call
// its methods; a page gives it a first text and what it wants to hear when a run ends.

import {Cell} from '../ui/cell';
import type {Keys} from '../ui/keys';
import {Editor} from './editor';
import {Entry} from './entry';
import type {Outcome} from './lessons';
import {serve} from './lsp';
import type {Diagnostic, Fix, Toolchain} from './protocol';
import {Runner} from './runner';
import {Source} from './source';
import {keep, kept} from './store';

const HELIX = 'vun.play.helix';
const FOLDED = 'vun.play.folded';

// How the status line says things are: nothing happening, a run going, or how one ended.
export type State = 'idle' | 'busy' | 'ok' | 'failed';
export type TabId = 'problems' | 'output';

export interface Options {
  code: string;
  // The text changed.
  changed: (code: string) => void;
  // A run ended as `outcome` says, of the `code` it was started on.
  ended: (outcome: Outcome, code: string) => void;
}

export class Bench {
  readonly status = new Cell('loading the compiler');
  readonly state = new Cell<State>('idle');
  readonly ready = new Cell(false);
  readonly running = new Cell(false);
  readonly tab = new Cell<TabId>('problems');
  readonly folded = new Cell(kept(FOLDED) === 'true');
  readonly helix = new Cell(kept(HELIX) === 'true');
  readonly entries = new Cell<Entry[]>([]);
  // Whether the build of the entries has ended, so that none means none and not yet.
  readonly built = new Cell(false);
  readonly output = new Cell('');
  readonly focused = new Cell<number | undefined>(undefined);
  readonly library = new Cell<Record<string, string>>({});
  // The fixes applied, and whether the others, made for the text as it was, are closed.
  readonly applied = new Cell<ReadonlySet<Fix>>(new Set());
  readonly stale = new Cell<ReadonlySet<Fix>>(new Set());
  readonly closed = new Cell(false);
  private editorView: Editor | undefined;
  private runner: Runner | undefined;
  private source = new Source('');

  constructor(
    readonly keys: Keys,
    private readonly loaded: Promise<Toolchain>,
    private readonly options: Options,
  ) {
    this.source = new Source(options.code);
    keys.on('run', () => void this.start());
    keys.on('helix', () => this.toggleHelix());
    keys.on('fold', () => (this.folded.value = !this.folded.value));
    keys.on('problems', () => this.show('problems'));
    keys.on('output', () => this.show('output'));
    keys.on('next-problem', () => this.editor.step(1));
    keys.on('previous-problem', () => this.editor.step(-1));
    keys.on('focus-editor', () => this.editor.view.focus());
  }

  get editor(): Editor {
    if (this.editorView === undefined) throw new Error('the workbench is not shown yet');
    return this.editorView;
  }

  // Draws the editor into `host`, and starts loading the compiler.
  mount(host: HTMLElement) {
    this.editorView = new Editor(host, this.options.code, this.options.changed, () => void this.start(), {
      over: (problem) => this.over(problem),
      pick: (problem) => this.pick(problem),
    });
    this.editorView.helix(this.helix.value);
    void this.loaded.then(
      (toolchain) => {
        this.runner = new Runner(toolchain);
        this.library.value = toolchain.library;
        this.ready.value = true;
        this.say('ready');
        const served = serve(toolchain, this.editor.code);
        if (served === undefined) return;
        this.editor.connect(served.transport);
        void served.ended.then((why) => this.say(`the language server stopped: ${why}`, 'failed'));
      },
      (error) => this.say(`the compiler did not load: ${error}`, 'failed'),
    );
  }

  say(text: string, state: State = 'idle') {
    this.status.value = text;
    this.state.value = state;
  }

  // Shows the tab `id`, unfolding the panel.
  show(id: TabId) {
    this.tab.value = id;
    this.folded.value = false;
    keep(FOLDED, 'false');
  }

  toggleFold() {
    this.folded.value = !this.folded.value;
    keep(FOLDED, String(this.folded.value));
  }

  toggleHelix() {
    this.helix.value = !this.helix.value;
    this.editor.helix(this.helix.value);
    keep(HELIX, String(this.helix.value));
    this.editor.view.focus();
  }

  // A build of `code` starts: what the last one found goes.
  private begin(code: string) {
    this.source = new Source(code);
    this.entries.value = [];
    this.built.value = false;
    this.focused.value = undefined;
    this.applied.value = new Set();
    this.stale.value = new Set();
    this.closed.value = false;
    this.editorView?.show([]);
  }

  private add(diagnostic: Diagnostic) {
    this.entries.value = [...this.entries.value, new Entry(diagnostic, this.entries.value.length, this.source)];
  }

  // The build ended: the editor shows where its problems point.
  private finish() {
    this.editor.show(this.entries.value.flatMap((entry) => entry.spans()));
    this.built.value = true;
  }

  // Puts `code` in the editor and clears what the last run showed.
  load(code: string) {
    if (this.runner?.running) this.runner.stop();
    this.running.value = false;
    this.editor.code = code;
    this.begin(code);
    this.output.value = '';
    this.say(this.ready.value ? 'ready' : 'loading the compiler');
  }

  // Lights the problem and its spans; none with `undefined`.
  light(problem: number | undefined) {
    if (problem === this.focused.value) return;
    this.focused.value = problem;
    this.editor.focus(problem);
  }

  // The pointer is over a span of the editor.
  over(problem: number | undefined) {
    this.light(problem);
    if (problem !== undefined) document.querySelector(`[data-problem="${problem}"]`)?.scrollIntoView({block: 'nearest', behavior: 'smooth'});
  }

  // A note in the editor was clicked.
  pick(problem: number) {
    this.show('problems');
    this.over(problem);
  }

  // Selects the first place of `diagnostic` in the editor, when the editor holds it.
  reveal(diagnostic: Diagnostic) {
    if (diagnostic.primary !== null) this.select(diagnostic.primary.at);
  }

  select(at: Diagnostic['labels'][number]['at']) {
    if (this.source.holds(at)) this.editor.select(this.source.from(at), this.source.to(at));
  }

  holds(at: Diagnostic['labels'][number]['at']): boolean {
    return this.source.holds(at);
  }

  // The lines `fix` changes, as the text would read after.
  patched(fix: Fix): string {
    const {source} = this;
    return source.patched(fix.edits.map((edit) => ({from: source.from(edit.at), to: source.to(edit.at), insert: edit.text})));
  }

  get text(): string {
    return this.source.text;
  }

  // Applies `fix` to the editor, if the editor still holds the text it was made for.
  apply(fix: Fix) {
    const {source} = this;
    if (this.editor.code !== source.text) {
      this.stale.value = new Set([...this.stale.value, fix]);
      return;
    }
    this.editor.change(fix.edits.map((edit) => ({from: source.from(edit.at), to: source.to(edit.at), insert: edit.text})));
    this.applied.value = new Set([...this.applied.value, fix]);
    this.closed.value = true;
  }

  // Runs the editor's code, or ends the run in progress.
  async start() {
    const {runner} = this;
    if (runner === undefined) return;
    if (runner.running) {
      runner.stop();
      this.running.value = false;
      this.say('stopped');
      return;
    }
    const code = this.editor.code;
    this.running.value = true;
    this.begin(code);
    this.output.value = '';
    let since = performance.now();
    let built = '';
    const seconds = () => `${((performance.now() - since) / 1000).toFixed(2)} s`;
    const outcome = await runner.run(code, {
      record: (record) => {
        if (record.type === 'diagnostic') this.add(record);
      },
      output: (text) => {
        this.output.value += text;
        this.show('output');
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
    if (!this.running.value) return;
    this.running.value = false;
    this.finish();
    if (outcome.diagnostics.length > 0) this.show('problems');
    else if (this.output.value !== '') this.show('output');
    this.options.ended(outcome, code);
  }
}
