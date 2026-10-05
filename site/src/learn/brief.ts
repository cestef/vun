// The column beside the editor: the lesson's words, what it asks as a list of checks that fill in
// as runs go, the help it can give, and the way through the course. A run is judged here, and a
// solved lesson says so and offers its reference solution to compare with.

import MarkdownIt from 'markdown-it';

import {type IconName, icon} from '../icons';
import {diff} from '../play/diff';
import {diffView} from '../play/diffview';
import {button, element} from '../play/dom';
import {type Lesson, type Outcome, type Standing, type Verdict, judge, tidy, verdicts} from '../play/lessons';
import type {Progress} from './progress';

const markdown = new MarkdownIt();

// Markdown with its links leaving the page, which is the course's.
function render(text: string, inline = false): string {
  const html = inline ? markdown.renderInline(text) : markdown.render(text);
  return html.replace(/<a href=/g, '<a target="_blank" rel="noreferrer" href=');
}

const MARKS: Record<Standing, IconName> = {pending: 'circle', met: 'check', missed: 'close'};

export class Brief {
  private readonly part: Record<'title' | 'text' | 'goal' | 'checks' | 'coach' | 'task' | 'tally' | 'summary' | 'list' | 'pips' | 'toggle', HTMLElement>;

  constructor(
    root: HTMLElement,
    private readonly course: Lesson[],
    private readonly progress: Progress,
    // The lesson at `index` was chosen from the map or the pips.
    private readonly choose: (index: number) => void,
  ) {
    const find = (name: string) => root.querySelector(`[data-${name}]`) as HTMLElement;
    this.part = {
      title: find('title'),
      text: find('text'),
      goal: find('goal'),
      checks: find('checks'),
      coach: find('coach'),
      task: find('task'),
      tally: find('tally'),
      summary: find('map-summary'),
      list: find('map-list'),
      pips: find('pips'),
      toggle: find('map-button'),
    };
  }

  get coach(): HTMLElement {
    return this.part.coach;
  }

  // Opens the list of lessons, or closes it.
  toggleMap(open = this.part.list.hidden) {
    this.part.list.hidden = !open;
    this.part.toggle.setAttribute('aria-expanded', String(open));
  }

  // Shows `lesson`, nothing run yet.
  render(lesson: Lesson) {
    const {part, progress} = this;
    part.title.textContent = lesson.title;
    part.text.innerHTML = render(lesson.text);
    part.goal.innerHTML = render(lesson.task, true);
    this.showChecks(lesson, undefined);
    part.task.classList.toggle('done', progress.isDone(lesson));
    this.refresh();
  }

  // The map, the pips and the way on, as the lessons done make them.
  refresh() {
    const {part, progress, course} = this;
    const lesson = progress.lesson;
    part.summary.textContent = `Lesson ${progress.index + 1} of ${course.length}`;
    part.pips.replaceChildren(
      ...course.map((other, i) => {
        const pip = button(`pip ${i === progress.index ? 'here' : ''} ${progress.isDone(other) ? 'done' : ''}`);
        pip.title = other.title;
        pip.setAttribute('aria-label', other.title);
        pip.addEventListener('click', () => this.choose(i));
        return pip;
      }),
    );
    part.list.replaceChildren(
      ...course.map((other, i) => {
        const link = button('lesson', element('span', 'mark', progress.isDone(other) ? icon('check') : String(i + 1)), other.title);
        link.addEventListener('click', () => {
          this.toggleMap(false);
          this.choose(i);
        });
        return element('li', `${i === progress.index ? 'here' : ''} ${progress.isDone(other) ? 'done' : ''}`, link);
      }),
    );
    const last = progress.index === course.length - 1;
    const next = document.querySelector<HTMLButtonElement>('[data-action="next-lesson"]');
    const previous = document.querySelector<HTMLButtonElement>('[data-action="previous-lesson"]');
    if (next !== null) {
      next.disabled = last;
      next.classList.toggle('ready', progress.isDone(lesson));
    }
    if (previous !== null) previous.disabled = progress.index === 0;
  }

  private showChecks(lesson: Lesson, outcome: Outcome | undefined) {
    if (lesson.expect === undefined) return;
    const list = verdicts(lesson.expect, outcome);
    this.part.checks.replaceChildren(...list.map((verdict) => this.check(verdict)));
    const met = list.filter((verdict) => verdict.standing === 'met').length;
    this.part.tally.textContent = `${met} of ${list.length}`;
    const wanted = lesson.expect.output;
    if (wanted !== undefined) this.part.checks.after(this.expected(wanted, outcome));
  }

  // The output asked for, a click away; once there is output that misses it, against what was printed.
  private expected(wanted: string, outcome: Outcome | undefined): HTMLElement {
    this.part.task.querySelector('.expected')?.remove();
    const printed = outcome === undefined || !outcome.built ? undefined : tidy(outcome.output);
    const wrong = printed !== undefined && printed !== tidy(wanted);
    const view = wrong ? diffView(diff(printed, tidy(wanted))) : element('pre', 'wanted', tidy(wanted));
    return element('details', 'expected', element('summary', '', icon('chevron-right'), wrong ? 'Compare with the expected output' : 'Expected output'), view);
  }

  private check(verdict: Verdict): HTMLElement {
    const row = element('li', `check ${verdict.standing}`, element('span', 'mark', icon(MARKS[verdict.standing])), element('span', 'name', verdict.name));
    if (verdict.cells !== undefined) row.append(element('span', 'strip', ...verdict.cells.map((cell, i) => Object.assign(element('i', cell), {title: `line ${i + 1}`}))));
    if (verdict.found !== undefined) row.append(element('span', 'found', verdict.found));
    return row;
  }

  // A run of `lesson` of `code` ended as `outcome` says. Returns whether the lesson is done.
  judged(lesson: Lesson, outcome: Outcome, code: string): boolean {
    if (lesson.expect === undefined) return false;
    this.showChecks(lesson, outcome);
    const done = judge(lesson.expect, outcome);
    if (done) this.solved(lesson, code);
    this.part.task.classList.toggle('done', done || this.progress.isDone(lesson));
    this.refresh();
    return done;
  }

  // The lesson is done: the reference solution beside what the learner wrote, a click away.
  private solved(lesson: Lesson, code: string) {
    this.progress.finish(lesson);
    const changes = diff(code, lesson.answer);
    const same = changes.every((change) => change.kind === 'same');
    const comparison = element('details', 'compare', element('summary', '', icon('check'), same ? 'Done. Your code is the reference solution.' : 'Done. Compare with the reference solution.'));
    if (!same) comparison.append(diffView(changes));
    this.part.coach.replaceChildren(comparison);
  }
}
