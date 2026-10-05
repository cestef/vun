// Lines of the editor marked from outside, such as the ones a hint points at, kept with their
// text through edits and cleared by an empty set.

import {StateEffect, StateField} from '@codemirror/state';
import {Decoration, type DecorationSet, EditorView} from '@codemirror/view';

export const markLines = StateEffect.define<number[]>();

const marked = Decoration.line({class: 'pointed'});

export const pointedLines = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(shown, transaction) {
    let next = shown.map(transaction.changes);
    for (const effect of transaction.effects) {
      if (!effect.is(markLines)) continue;
      const {doc} = transaction.state;
      next = Decoration.set(effect.value.filter((n) => n >= 1 && n <= doc.lines).map((n) => marked.range(doc.line(n).from)));
    }
    return next;
  },
  provide: (field) => EditorView.decorations.from(field),
});
