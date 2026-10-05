// The problems of the last build: a card for each diagnostic, in the order the compiler said them,
// and what ties them to the editor. A card and the spans it owns light each other as the pointer
// reaches either, and a click on a span's note brings the card forward.

import {element} from './dom';
import type {Editor} from './editor';
import {Problem} from './problem';
import type {Diagnostic} from './protocol';
import {Source} from './source';

export class Problems {
  private cards: Problem[] = [];
  private source = new Source('');
  private focused: number | undefined;
  // The library the explanation pages are read from, once it is loaded.
  private library: Record<string, string> = {};

  constructor(
    readonly view: HTMLElement,
    private readonly editor: Editor,
    // Tells the dock the list has new things to show, and how many.
    private readonly counted: (errors: number, warnings: number) => void,
    // Called to bring the list forward, as a click on a note in the editor asks.
    private readonly forward: () => void,
  ) {}

  use(library: Record<string, string>) {
    this.library = library;
  }

  // A build of `code` starts: what the last one found goes.
  start(code: string) {
    this.source = new Source(code);
    this.cards = [];
    this.focused = undefined;
    this.view.replaceChildren(element('p', 'none', 'Nothing to report yet.'));
    this.editor.show([]);
    this.counted(0, 0);
  }

  add(diagnostic: Diagnostic) {
    const card = new Problem(diagnostic, this.cards.length, this.source, this.editor, this.library, () => this.cards.forEach((c) => c.close()));
    if (this.cards.length === 0) this.view.replaceChildren();
    this.cards.push(card);
    card.root.addEventListener('mouseenter', () => this.light(card.index));
    card.root.addEventListener('mouseleave', () => this.light(undefined));
    this.view.append(card.root);
  }

  // The build ended: the editor shows where its problems point.
  finish() {
    this.editor.show(this.cards.flatMap((card) => card.spans()));
    const count = (severity: string) => this.cards.filter((card) => card.root.classList.contains(severity)).length;
    this.counted(count('error'), count('warning'));
    if (this.cards.length === 0) this.view.replaceChildren(element('p', 'none', 'No problems.'));
  }

  // Lights the card of `problem` and its spans; none with `undefined`.
  light(problem: number | undefined) {
    if (problem === this.focused) return;
    this.focused = problem;
    for (const card of this.cards) card.focus(card.index === problem);
    this.editor.focus(problem);
  }

  // The pointer is over a span of the editor.
  over(problem: number | undefined) {
    this.light(problem);
    if (problem !== undefined) this.cards[problem]?.root.scrollIntoView({block: 'nearest', behavior: 'smooth'});
  }

  // A note in the editor was clicked.
  pick(problem: number) {
    this.forward();
    this.light(problem);
    this.cards[problem]?.root.scrollIntoView({block: 'nearest', behavior: 'smooth'});
  }
}
