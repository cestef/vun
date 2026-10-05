// The places a build's diagnostics point at, shown in the editor: each label's range marked, its
// message after its line while its problem is in focus, a mark kept with the text as it is edited. A span belongs to a problem,
// and the problem the pointer is over, or the report has focused, is drawn stronger. Moving between
// problems is by their primary spans, in the order of the text.

import {type Extension, RangeSetBuilder, StateEffect, StateField} from '@codemirror/state';
import {Decoration, type DecorationSet, EditorView, WidgetType, keymap} from '@codemirror/view';

export type Role = 'primary' | 'label';

export interface Span {
  // The problem it belongs to, as the report numbers them.
  problem: number;
  role: Role;
  severity: string;
  from: number;
  to: number;
  // What the label says, shown after its line.
  note: string | null;
}

// Which problem is drawn stronger, if one is.
export interface Held {
  spans: Span[];
  focus: number | undefined;
}

export const showSpans = StateEffect.define<Span[]>();
export const focusProblem = StateEffect.define<number | undefined>();

// What the editor tells the page about the spans: the pointer over a problem's, a click on a note.
export interface Pointing {
  over(problem: number | undefined): void;
  pick(problem: number): void;
}

class Note extends WidgetType {
  constructor(private readonly span: Span, private readonly pointing: Pointing) {
    super();
  }

  eq(other: Note) {
    return other.span === this.span;
  }

  toDOM() {
    const note = document.createElement('button');
    note.type = 'button';
    note.className = `span-note ${this.span.role} ${this.span.severity}`;
    note.textContent = this.span.note;
    note.addEventListener('mousedown', (event) => event.preventDefault());
    note.addEventListener('click', () => this.pointing.pick(this.span.problem));
    note.addEventListener('mouseenter', () => this.pointing.over(this.span.problem));
    note.addEventListener('mouseleave', () => this.pointing.over(undefined));
    return note;
  }

  ignoreEvent() {
    return false;
  }
}

function decorations(held: Held, state: {doc: {lineAt(at: number): {to: number}; length: number}}, pointing: Pointing): DecorationSet {
  const marks: {from: number; to: number; value: Decoration}[] = [];
  for (const span of held.spans) {
    const strong = span.problem === held.focus ? ' focused' : '';
    if (span.to > span.from) {
      marks.push({
        from: span.from,
        to: span.to,
        value: Decoration.mark({class: `span ${span.role} ${span.severity}${strong}`, attributes: {'data-problem': String(span.problem)}}),
      });
    }
    if (span.note !== null && strong && span.from <= state.doc.length) {
      const at = state.doc.lineAt(Math.min(span.to, state.doc.length)).to;
      marks.push({from: at, to: at, value: Decoration.widget({widget: new Note(span, pointing), side: 1})});
    }
  }
  marks.sort((a, b) => a.from - b.from || a.to - b.to);
  const builder = new RangeSetBuilder<Decoration>();
  for (const {from, to, value} of marks) builder.add(from, to, value);
  return builder.finish();
}

export type SpansState = {held: Held; shown: DecorationSet};

export function spansField(pointing: Pointing) {
  return StateField.define<SpansState>({
    create: () => ({held: {spans: [], focus: undefined}, shown: Decoration.none}),
    update(value, transaction) {
      let {held} = value;
      let changed = false;
      if (transaction.docChanged) {
        const spans = held.spans.map((span) => ({...span, from: transaction.changes.mapPos(span.from, 1), to: transaction.changes.mapPos(span.to, -1)}));
        held = {...held, spans: spans.map((span) => ({...span, to: Math.max(span.from, span.to)}))};
        changed = true;
      }
      for (const effect of transaction.effects) {
        if (effect.is(showSpans)) held = {spans: effect.value, focus: undefined};
        else if (effect.is(focusProblem)) held = {...held, focus: effect.value};
        else continue;
        changed = true;
      }
      return changed ? {held, shown: decorations(held, transaction.state, pointing)} : value;
    },
    provide: (field) => EditorView.decorations.from(field, (value) => value.shown),
  });
}

// The spans, the pointer over them told to `pointing`, and the keys that move between problems.
export function spansExtension(pointing: Pointing, field: ReturnType<typeof spansField>, move: (step: 1 | -1) => boolean): Extension {
  return [
    field,
    EditorView.domEventHandlers({
      mouseover: (event) => {
        const hit = (event.target as HTMLElement).closest?.('[data-problem]');
        pointing.over(hit === null || hit === undefined ? undefined : Number((hit as HTMLElement).dataset.problem));
        return false;
      },
      mouseleave: () => {
        pointing.over(undefined);
        return false;
      },
    }),
    keymap.of([
      {key: 'F8', run: () => move(1)},
      {key: 'Shift-F8', run: () => move(-1)},
    ]),
  ];
}
