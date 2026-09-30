// What a run reports, drawn from the compiler's records rather than its terminal text: each
// diagnostic a card, its message's code and types set apart, its place a link into the editor,
// the source under it marked where its labels point, its notes and helps, its fixes applied to the
// editor with a click and its explanation page a click away.

import MarkdownIt from 'markdown-it';

import type {Editor} from './editor';
import {coloredBlocks} from './highlight';
import type {At, Diagnostic, Fix, Label, Message, Reported} from './protocol';
import {ROOT} from './wasi';

// How many lines around its labels an excerpt shows at most.
const EXCERPT = 8;

const markdown = new MarkdownIt();
const encoder = new TextEncoder();
const decoder = new TextDecoder();

function element<K extends keyof HTMLElementTagNameMap>(tag: K, className = '', ...children: (Node | string)[]) {
  const made = document.createElement(tag);
  made.className = className;
  made.append(...children);
  return made;
}

// A message's parts, code and types in `<code>`.
function message(msg: Message): (Node | string)[] {
  return msg.parts.map((part) => (part.kind === 'text' ? part.value : element('code', `part ${part.kind}`, part.value)));
}

// The explanation page of the diagnostic `path` names in the library: its package's directory,
// then the path's names as directories, as the library lays packages out.
function page(library: Record<string, string>, path: string): string | undefined {
  const names = path.split('.');
  return library[`${names[0]}/${names.join('/')}.md`];
}

export class Report {
  private readonly root: HTMLElement;
  // The text built, which the records' offsets count bytes of.
  private code = '';
  private bytes = new Uint8Array();
  private program: HTMLPreElement | undefined;

  constructor(root: HTMLElement, private readonly editor: Editor, private readonly library: Record<string, string>) {
    this.root = root;
  }

  start(code: string) {
    this.code = code;
    this.bytes = encoder.encode(code);
    this.program = undefined;
    this.root.replaceChildren();
  }

  record(record: Reported) {
    if (record.type === 'diagnostic') this.root.append(this.card(record));
    else if (record.type === 'finished') {
      const counts = [record.errors && `${record.errors} error${record.errors > 1 ? 's' : ''}`, record.warnings && `${record.warnings} warning${record.warnings > 1 ? 's' : ''}`].filter(Boolean);
      const said = record.success ? 'built' : 'failed';
      this.root.append(element('p', `summary ${said}`, `${said} in ${record.milliseconds} ms`, counts.length > 0 ? `, ${counts.join(', ')}` : ''));
    }
  }

  // What the program or a compiler that wrote no record wrote.
  output(text: string) {
    this.program ??= this.root.appendChild(element('pre', 'program'));
    this.program.append(text);
    this.root.scrollTop = this.root.scrollHeight;
  }

  // The character offset of the byte offset `at` of the text built.
  private char(at: number): number {
    return decoder.decode(this.bytes.subarray(0, at)).length;
  }

  private inRoot(at: At): boolean {
    return at.file === ROOT || at.file.endsWith(`/${ROOT}`);
  }

  private card(d: Diagnostic): HTMLElement {
    const head = element('header', '', element('span', `severity ${d.severity}`, d.severity), element('span', 'message', ...message(d.message)));
    const card = element('article', `diagnostic ${d.severity}`, head);
    const explained = page(this.library, d.path);
    const id = element('button', 'id', d.id);
    id.type = 'button';
    id.disabled = explained === undefined;
    head.append(id);
    const at = d.primary?.at;
    if (at !== undefined) card.append(this.place(at));
    const labels = [...(d.primary ? [d.primary] : []), ...d.labels].filter((label) => this.inRoot(label.at));
    if (labels.length > 0) card.append(this.excerpt(labels, d.severity));
    for (const child of d.children) {
      card.append(element('p', `child ${child.kind}`, element('b', '', child.kind), ': ', ...message(child.message)));
      for (const fix of child.fixes) card.append(this.fix(fix));
    }
    for (const fix of d.fixes) card.append(this.fix(fix));
    if (explained !== undefined) {
      const page = element('div', 'page');
      page.hidden = true;
      id.addEventListener('click', () => {
        if (page.childElementCount === 0) page.innerHTML = coloredBlocks(markdown.render(explained));
        page.hidden = !page.hidden;
        id.classList.toggle('open', !page.hidden);
      });
      card.append(page);
    }
    return card;
  }

  // Where `at` is, a link selecting it in the editor when it is in the root file.
  private place(at: At): HTMLElement {
    const shown = `${at.file}:${at.start.line}:${at.start.column}`;
    if (!this.inRoot(at)) return element('p', 'place', shown);
    const link = element('button', 'place', shown);
    link.type = 'button';
    link.addEventListener('click', () => this.editor.select(this.char(at.bytes.start), this.char(at.bytes.end)));
    return element('p', '', link);
  }

  // The lines `labels` point into, each range marked, each label's message under its line.
  private excerpt(labels: Label[], severity: string): HTMLElement {
    const lines = this.code.split('\n');
    const first = Math.min(...labels.map((l) => l.at.start.line));
    const last = Math.min(Math.max(...labels.map((l) => l.at.end.line)), first + EXCERPT - 1);
    const table = element('div', 'excerpt');
    const starts = lines.reduce<number[]>((all, line, i) => (all.push(i === 0 ? 0 : all[i - 1] + lines[i - 1].length + 1), all), []);
    for (let number = first; number <= last; number++) {
      const text = lines[number - 1] ?? '';
      const start = starts[number - 1] ?? 0;
      // The labels' ranges on this line, as character columns.
      const marks = labels
        .map((label, i) => ({label, primary: i === 0 && labels[0] === label, from: this.char(label.at.bytes.start) - start, to: this.char(label.at.bytes.end) - start}))
        .filter((m) => m.to >= 0 && m.from <= text.length)
        .map((m) => ({...m, from: Math.max(m.from, 0), to: Math.min(Math.max(m.to, m.from + 1), text.length + 1)}))
        .sort((a, b) => a.from - b.from);
      const code = element('span', 'code');
      let at = 0;
      for (const mark of marks) {
        if (mark.from < at) continue;
        code.append(text.slice(at, mark.from), element('span', mark.primary ? `range primary ${severity}` : 'range label', text.slice(mark.from, mark.to) || ' '));
        at = mark.to;
      }
      code.append(text.slice(at));
      table.append(element('span', 'number', String(number)), code);
      for (const mark of marks.filter((m) => m.label.message !== null && m.label.at.end.line === number)) {
        const note = element('span', mark.primary ? `note primary ${severity}` : 'note label', ...message(mark.label.message as Message));
        note.style.paddingLeft = `${mark.from}ch`;
        table.append(element('span', 'number'), note);
      }
    }
    return table;
  }

  // A button applying `fix` to the editor, while the editor holds the text built.
  private fix(fix: Fix): HTMLElement {
    const applied = element('span', 'apply', 'apply');
    const button = element('button', 'fix', applied, ...message(fix.message));
    button.type = 'button';
    if (fix.safety !== 'safe') button.append(element('span', `safety ${fix.safety}`, fix.safety));
    button.addEventListener('click', () => {
      if (this.editor.code !== this.code) {
        button.title = 'the code changed since this build';
        button.classList.add('stale');
        return;
      }
      this.editor.change(fix.edits.map((edit) => ({from: this.char(edit.at.bytes.start), to: this.char(edit.at.bytes.end), insert: edit.text})));
      for (const other of this.root.querySelectorAll<HTMLButtonElement>('.fix')) other.disabled = true;
      button.classList.add('applied');
      applied.textContent = 'applied';
    });
    return button;
  }
}
