// The help a lesson gives, a rung at a time, each telling more than the one before: the hint, the
// lines of the code that differ from a working solution, the change to make, and the solution
// itself. Asking for a rung costs nothing but is never shown unasked.

import {diff, differing} from '../play/diff';
import {diffView} from '../play/diffview';
import {element} from '../play/dom';
import type {Lesson} from '../play/lessons';

// What the coach reads and does in the editor.
export interface Hands {
  code(): string;
  point(lines: number[]): void;
  replace(code: string): void;
}

interface Rung {
  // What asking for it is called.
  ask: string;
  show(lesson: Lesson, hands: Hands): HTMLElement;
}

const RUNGS: Rung[] = [
  {ask: 'Give me a hint', show: (lesson) => element('p', 'hint', lesson.hint)},
  {
    ask: 'Where should I look?',
    show: (lesson, hands) => {
      const lines = differing(hands.code(), lesson.answer);
      hands.point(lines);
      return element('p', 'hint', lines.length === 0 ? 'Your code and a working solution do not differ.' : (lines.length > 1 ? 'The marked lines in the editor differ from a working solution.' : 'The marked line in the editor differs from a working solution.'));
    },
  },
  {
    ask: 'Show me the change',
    show: (lesson, hands) => {
      const changes = diff(hands.code(), lesson.answer);
      const view = diffView(changes);
      return element('div', 'change', element('p', 'hint', 'What a working solution changes in your code:'), view);
    },
  },
  {
    ask: 'Put the answer in the editor',
    show: (lesson, hands) => {
      hands.replace(lesson.answer);
      return element('p', 'hint', 'The answer is in the editor. Undo brings your code back.');
    },
  },
];

export class Coach {
  private level = 0;

  constructor(
    private readonly view: HTMLElement,
    private readonly ask: HTMLButtonElement,
    private readonly asking: HTMLElement,
    // One mark for each rung, filled as far as the learner has climbed.
    private readonly rungs: HTMLElement,
    private readonly hands: Hands,
  ) {
    this.rungs.replaceChildren(...RUNGS.map(() => element('li')));
  }

  // A fresh lesson, or a solved one: nothing is shown and the first rung is next.
  reset() {
    this.level = 0;
    this.view.replaceChildren();
    this.hands.point([]);
    this.refresh();
  }

  // Shows the next rung of `lesson`'s help.
  next(lesson: Lesson) {
    if (this.level >= RUNGS.length) return;
    this.view.replaceChildren(RUNGS[this.level].show(lesson, this.hands));
    this.level++;
    this.refresh();
  }

  private refresh() {
    this.ask.disabled = this.level >= RUNGS.length;
    this.asking.textContent = this.level >= RUNGS.length ? 'No more help' : RUNGS[this.level].ask;
    for (const [i, rung] of [...this.rungs.children].entries()) rung.classList.toggle('used', i < this.level);
  }
}
