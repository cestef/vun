// What a learner has done, kept between visits: the lesson they are on, each lesson's code as it
// was left, and which are done. The keys are the playground's own, so earlier visits carry over.

import type {Lesson} from '../play/lessons';
import {keep, kept} from '../play/store';

const CURRENT = 'vun.play.lesson';
const DONE = 'vun.play.done';
const codeKey = (lesson: Lesson) => `vun.play.code.${lesson.id}`;

export class Progress {
  private done = new Set<string>();
  index = 0;

  constructor(private readonly course: Lesson[]) {
    try {
      this.done = new Set(JSON.parse(kept(DONE) ?? '[]') as string[]);
    } catch {
      // Nothing is done, as at a first visit.
    }
    this.go(Math.max(0, course.findIndex((lesson) => lesson.id === kept(CURRENT))));
  }

  get lesson(): Lesson {
    return this.course[this.index];
  }

  // The lesson `id` names, or none.
  find(id: string): number {
    return this.course.findIndex((lesson) => lesson.id === id);
  }

  go(index: number) {
    if (index < 0 || index >= this.course.length) return;
    this.index = index;
    keep(CURRENT, this.lesson.id);
  }

  // The lesson's code as it was left, or as it starts.
  code(lesson = this.lesson): string {
    return kept(codeKey(lesson)) ?? lesson.start;
  }

  keepCode(code: string) {
    keep(codeKey(this.lesson), code);
  }

  isDone(lesson: Lesson): boolean {
    return this.done.has(lesson.id);
  }

  get finished(): number {
    return this.course.filter((lesson) => this.done.has(lesson.id)).length;
  }

  finish(lesson: Lesson) {
    this.done.add(lesson.id);
    keep(DONE, JSON.stringify([...this.done]));
  }
}
