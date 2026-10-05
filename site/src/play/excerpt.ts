// The lines a diagnostic's labels point into, as the report shows them: each range marked and
// clickable, each label's message under its line, with the way to a label on another line.

import {button, element, message} from './dom';
import type {Label, Message} from './protocol';
import type {Source} from './source';

// How many lines around its labels an excerpt shows at most.
const EXCERPT = 8;

// Selects a label's range in the editor.
export type Jump = (label: Label) => void;

interface Mark {
  label: Label;
  primary: boolean;
  from: number;
  to: number;
}

export function excerpt(source: Source, labels: Label[], severity: string, jump: Jump): HTMLElement {
  const lines = source.lines();
  const first = Math.min(...labels.map((l) => l.at.start.line));
  const last = Math.min(Math.max(...labels.map((l) => l.at.end.line)), first + EXCERPT - 1);
  const table = element('div', 'excerpt');
  for (let number = first; number <= last; number++) {
    const text = lines[number - 1] ?? '';
    const start = source.start(number);
    const marks: Mark[] = labels
      .map((label, i) => ({label, primary: i === 0, from: source.from(label.at) - start, to: source.to(label.at) - start}))
      .filter((m) => m.to >= 0 && m.from <= text.length)
      .map((m) => ({...m, from: Math.max(m.from, 0), to: Math.min(Math.max(m.to, m.from + 1), text.length + 1)}))
      .sort((a, b) => a.from - b.from);
    const code = element('span', 'code');
    let at = 0;
    for (const mark of marks) {
      if (mark.from < at) continue;
      const range = button(mark.primary ? `range primary ${severity}` : 'range label', text.slice(mark.from, mark.to) || ' ');
      range.title = 'show in the editor';
      range.addEventListener('click', () => jump(mark.label));
      code.append(text.slice(at, mark.from), range);
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
  // A label past the lines shown is a way to it, not a missing one.
  for (const label of labels.filter((l) => l.at.start.line > last || l.at.end.line < first)) {
    const there = button('far', `line ${label.at.start.line}`, ...(label.message === null ? [] : [': ', ...message(label.message)]));
    there.addEventListener('click', () => jump(label));
    table.append(element('span', 'number'), there);
  }
  return table;
}
