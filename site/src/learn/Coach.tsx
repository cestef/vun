// The help a lesson gives, a rung at a time, each telling more than the one before: the hint, the
// lines of the code that differ from a working solution, the change to make, and the solution
// itself. Asking for a rung costs nothing but is never shown unasked.

import type {JSX} from '@solidjs/web';
import {For, Show} from 'solid-js';

import type {Bench} from '../play/bench';
import {diff, differing} from '../play/diff';
import {Action} from '../ui/Keycaps';
import {Cell} from '../ui/cell';
import {DiffView} from '../ui/DiffView';

export interface Material {
  hint: string;
  answer: string;
}

interface Rung {
  // What asking for it is called.
  ask: string;
  show(lesson: Material, bench: Bench): JSX.Element;
}

const RUNGS: Rung[] = [
  {ask: 'Give me a hint', show: (lesson) => <p class="hint">{lesson.hint}</p>},
  {
    ask: 'Where should I look?',
    show: (lesson, bench) => {
      const lines = differing(bench.editor.code, lesson.answer);
      bench.editor.point(lines);
      const said = lines.length === 0 ? 'Your code and a working solution do not differ.' : `The marked ${lines.length > 1 ? 'lines' : 'line'} in the editor ${lines.length > 1 ? 'differ' : 'differs'} from a working solution.`;
      return <p class="hint">{said}</p>;
    },
  },
  {
    ask: 'Show me the change',
    show: (lesson, bench) => (
      <div class="change">
        <p class="hint">What a working solution changes in your code:</p>
        <DiffView changes={diff(bench.editor.code, lesson.answer)} />
      </div>
    ),
  },
  {
    ask: 'Put the answer in the editor',
    show: (lesson, bench) => {
      bench.editor.code = lesson.answer;
      return <p class="hint">The answer is in the editor. Undo brings your code back.</p>;
    },
  },
];

export class Ladder {
  readonly level = new Cell(0);
  readonly shown = new Cell<JSX.Element>(undefined);

  constructor(
    private readonly lesson: Material,
    private readonly bench: Bench,
  ) {}

  get asking(): string {
    return this.level.value >= RUNGS.length ? 'No more help' : RUNGS[this.level.value].ask;
  }

  // Shows the next rung.
  next() {
    if (this.level.value >= RUNGS.length) return;
    this.shown.value = RUNGS[this.level.value].show(this.lesson, this.bench);
    this.level.value += 1;
  }

  // A fresh start, or a solved lesson: nothing is shown and the first rung is next.
  reset(shown?: JSX.Element) {
    this.level.value = 0;
    this.shown.value = shown;
    this.bench.editor.point([]);
  }
}

export function Coach(props: {ladder: Ladder; bench: Bench}) {
  return (
    <section class="coaching">
      <Action keys={props.bench.keys} action="hint" class="ask" icon="bulb" caps disabled={props.ladder.level.value >= RUNGS.length}>
        {props.ladder.asking}
      </Action>
      <ol class="rungs" aria-hidden="true">
        <For each={RUNGS}>{(_, i) => <li class={{used: i() < props.ladder.level.value}}></li>}</For>
      </ol>
      <div class="coach">
        <Show when={props.ladder.shown.value}>{(shown) => shown()}</Show>
      </div>
    </section>
  );
}
