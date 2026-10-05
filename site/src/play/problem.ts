// One diagnostic as the report shows it: its message and id, where it is, the source under it with
// its labels, its notes and helps, its fixes (each previewed before it is applied) and its
// explanation page. It also says which spans of the editor it owns.

import MarkdownIt from 'markdown-it';

import {icon} from '../icons';
import {diff} from './diff';
import {diffView} from './diffview';
import {button, element, message} from './dom';
import type {Editor} from './editor';
import {excerpt} from './excerpt';
import {coloredBlocks} from './highlight';
import type {At, Diagnostic, Fix, Label} from './protocol';
import type {Source} from './source';
import type {Role, Span} from './spans';

const markdown = new MarkdownIt();

// The explanation page of the diagnostic `path` names in the library: its package's directory,
// then the path's names as directories, as the library lays packages out.
function page(library: Record<string, string>, path: string): string | undefined {
  const names = path.split('.');
  return library[`${names[0]}/${names.join('/')}.md`];
}

export class Problem {
  readonly root: HTMLElement;
  private readonly fixes: HTMLButtonElement[] = [];

  constructor(
    private readonly diagnostic: Diagnostic,
    readonly index: number,
    private readonly source: Source,
    private readonly editor: Editor,
    library: Record<string, string>,
    // Called when a fix is applied, so that the others, made for the text as it was, are closed.
    private readonly applied: () => void,
  ) {
    const d = diagnostic;
    const head = button('head', icon(d.severity === 'error' ? 'alert' : 'warning'), element('span', 'message', ...message(d.message)));
    head.title = 'show in the editor';
    head.addEventListener('click', () => this.reveal());
    const id = button('id', d.id);
    const explained = page(library, d.path);
    id.disabled = explained === undefined;
    this.root = element('article', `problem ${d.severity}`, element('header', '', head, id));
    this.root.dataset.problem = String(index);
    const at = d.primary?.at;
    if (at !== undefined) this.root.append(this.place(at));
    const labels = [...(d.primary ? [d.primary] : []), ...d.labels].filter((label) => source.holds(label.at));
    if (labels.length > 0) this.root.append(excerpt(source, labels, d.severity, (label) => this.select(label)));
    for (const child of d.children) {
      this.root.append(element('p', `child ${child.kind}`, element('b', '', child.kind), ': ', ...message(child.message)));
      for (const fix of child.fixes) this.root.append(this.fix(fix));
    }
    for (const fix of d.fixes) this.root.append(this.fix(fix));
    if (explained !== undefined) this.root.append(this.explanation(id, explained));
  }

  // Where this problem is drawn in the editor.
  spans(): Span[] {
    const {diagnostic: d, source} = this;
    const spans: Span[] = [];
    const add = (label: Label | null, role: Role) => {
      if (label === null || !source.holds(label.at)) return;
      spans.push({problem: this.index, role, severity: d.severity, from: source.from(label.at), to: source.to(label.at), note: label.message?.text ?? null});
    };
    add(d.primary, 'primary');
    for (const label of d.labels) add(label, 'label');
    return spans;
  }

  focus(on: boolean) {
    this.root.classList.toggle('focused', on);
  }

  // Selects the primary place in the editor.
  reveal() {
    const at = this.diagnostic.primary;
    if (at !== null) this.select(at);
  }

  private select(label: Label) {
    if (this.source.holds(label.at)) this.editor.select(this.source.from(label.at), this.source.to(label.at));
  }

  private place(at: At): HTMLElement {
    const shown = `${at.file}:${at.start.line}:${at.start.column}`;
    if (!this.source.holds(at)) return element('p', 'place', shown);
    const link = button('place', shown);
    link.addEventListener('click', () => this.reveal());
    return element('p', '', link);
  }

  private explanation(id: HTMLElement, text: string): HTMLElement {
    const shown = element('div', 'page');
    shown.hidden = true;
    id.addEventListener('click', () => {
      if (shown.childElementCount === 0) shown.innerHTML = coloredBlocks(markdown.render(text));
      shown.hidden = !shown.hidden;
      id.classList.toggle('open', !shown.hidden);
    });
    return shown;
  }

  // The lines `fix` changes, as the text would read after.
  private preview(fix: Fix): HTMLElement {
    const {source} = this;
    const after = source.patched(fix.edits.map((edit) => ({from: source.from(edit.at), to: source.to(edit.at), insert: edit.text})));
    return diffView(diff(source.text, after));
  }

  // A fix: what it says and `apply`, with its change shown under it first.
  private fix(fix: Fix): HTMLElement {
    const applied = element('span', 'apply', icon('wand'), 'Apply');
    const apply = button('fix', applied, ...message(fix.message));
    if (fix.safety !== 'safe') apply.append(element('span', `safety ${fix.safety}`, fix.safety));
    const preview = this.preview(fix);
    const shown = element('div', 'fixing', apply, preview);
    apply.addEventListener('click', () => {
      if (this.editor.code !== this.source.text) {
        apply.title = 'the code changed since this build';
        apply.classList.add('stale');
        return;
      }
      const {source} = this;
      this.editor.change(fix.edits.map((edit) => ({from: source.from(edit.at), to: source.to(edit.at), insert: edit.text})));
      this.applied();
      apply.classList.add('applied');
      applied.replaceChildren(icon('check'), 'Applied');
    });
    this.fixes.push(apply);
    return shown;
  }

  // Closes this problem's fixes, which were made for text that has changed.
  close() {
    for (const fix of this.fixes) if (!fix.classList.contains('applied')) fix.disabled = true;
  }
}
