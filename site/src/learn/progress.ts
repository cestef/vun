// What a learner has done, kept between visits: each lesson's code as it was left, and which are
// done. The keys are the playground's own, so earlier visits carry over.

import {keep, kept} from '../play/store';
import {Cell} from '../ui/cell';

const DONE = 'vun.play.done';
const codeKey = (lesson: string) => `vun.play.code.${lesson}`;

export class Progress {
  readonly done = new Cell<ReadonlySet<string>>(new Set());

  constructor() {
    try {
      this.done.value = new Set(JSON.parse(kept(DONE) ?? '[]') as string[]);
    } catch {
      // Nothing is done, as at a first visit.
    }
  }

  // The lesson's code as it was left, or as it starts.
  code(lesson: string, start: string): string {
    return kept(codeKey(lesson)) ?? start;
  }

  keepCode(lesson: string, code: string) {
    keep(codeKey(lesson), code);
  }

  finish(lesson: string) {
    this.done.value = new Set([...this.done.value, lesson]);
    keep(DONE, JSON.stringify([...this.done.value]));
  }
}
