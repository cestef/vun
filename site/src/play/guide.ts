// The guide beside the editor: one lesson at a time, with what to do, a hint and the answer a click
// away, and a mark once a run has done what the lesson asks. What it keeps (the lesson, each
// lesson's code, which are done) it keeps between visits, so a tour can be left and picked up.

import MarkdownIt from 'markdown-it';

import {type Expect, type Lesson, type Outcome, judge} from './lessons';
import {keep, kept} from './store';

const CURRENT = 'vun.play.lesson';
const DONE = 'vun.play.done';
const codeKey = (lesson: Lesson) => `vun.play.code.${lesson.id}`;

const markdown = new MarkdownIt();

// Markdown with its links leaving the page, which is the tour's.
function render(text: string, inline = false): string {
  const html = inline ? markdown.renderInline(text) : markdown.render(text);
  return html.replace(/<a href=/g, '<a target="_blank" rel="noreferrer" href=');
}

// What a lesson wants, as a line.
function wants(expect: Expect): string {
  if (expect.error !== undefined) return `the compiler should refuse it with ${expect.error}`;
  if (expect.output !== undefined) return 'it should print';
  return 'it should end normally';
}

// The lines of `text` as `pre` shows them.
const tidy = (text: string) => text.replace(/[ \t]+$/gm, '').trim();

export class Guide {
  private index = 0;
  private done = new Set<string>();
  private readonly shown: Record<'steps' | 'title' | 'text' | 'task' | 'goal' | 'hint' | 'expected', HTMLElement>;
  private readonly buttons: Record<'hint' | 'answer' | 'reset' | 'previous' | 'next', HTMLButtonElement>;

  // `show` puts code in the editor.
  constructor(root: HTMLElement, private readonly lessons: Lesson[], private readonly show: (code: string) => void) {
    const find = <T extends HTMLElement>(name: string) => root.querySelector(`[data-guide-${name}]`) as T;
    this.shown = {
      steps: find('steps'),
      title: find('title'),
      text: find('text'),
      task: find('task'),
      goal: find('goal'),
      hint: find('hint'),
      expected: find('expected'),
    };
    this.buttons = {
      hint: find('hint-button'),
      answer: find('answer'),
      reset: find('reset'),
      previous: find('previous'),
      next: find('next'),
    };
    try {
      this.done = new Set(JSON.parse(kept(DONE) ?? '[]') as string[]);
    } catch {
      // Nothing is done, as at a first visit.
    }
    const found = lessons.findIndex((lesson) => lesson.id === kept(CURRENT));
    this.index = found < 0 ? 0 : found;

    this.shown.steps.append(
      ...lessons.map((lesson, i) => {
        const step = Object.assign(document.createElement('button'), {type: 'button', textContent: String(i + 1), title: lesson.title});
        step.setAttribute('role', 'tab');
        step.addEventListener('click', () => this.go(i));
        return step;
      }),
    );
    this.buttons.hint.addEventListener('click', () => this.shown.hint.toggleAttribute('hidden'));
    this.buttons.answer.addEventListener('click', () => this.show(this.lesson.answer));
    this.buttons.reset.addEventListener('click', () => this.show(this.lesson.start));
    this.buttons.previous.addEventListener('click', () => this.go(this.index - 1));
    this.buttons.next.addEventListener('click', () => this.go(this.index + 1));
  }

  get lesson(): Lesson {
    return this.lessons[this.index];
  }

  // The code the editor starts with: the lesson's as it was left, or as it starts.
  get opening(): string {
    return kept(codeKey(this.lesson)) ?? this.lesson.start;
  }

  // Opens the last lesson, which is free, with `code`: what a shared link holds.
  free(code: string) {
    this.index = this.lessons.length - 1;
    keep(codeKey(this.lesson), code);
  }

  // The editor's code changed.
  changed(code: string) {
    keep(codeKey(this.lesson), code);
  }

  // Shows the lesson `index`, with its code as it was left.
  go(index: number) {
    if (index < 0 || index >= this.lessons.length) return;
    this.index = index;
    keep(CURRENT, this.lesson.id);
    this.show(this.opening);
    this.render();
  }

  render() {
    const {lesson} = this;
    const task = lesson.expect !== undefined;
    this.shown.title.textContent = lesson.title;
    this.shown.text.innerHTML = render(lesson.text);
    this.shown.task.hidden = !task;
    this.shown.goal.innerHTML = render(lesson.task, true);
    this.shown.hint.innerHTML = render(lesson.hint, true);
    this.shown.hint.hidden = true;
    this.shown.expected.hidden = true;
    for (const button of [this.buttons.hint, this.buttons.answer, this.buttons.reset]) button.hidden = !task && button !== this.buttons.reset;
    this.buttons.previous.disabled = this.index === 0;
    this.buttons.next.disabled = this.index === this.lessons.length - 1;
    this.refresh();
  }

  // What a run can change: the lessons done, their marks, and the way on pointed out.
  private refresh() {
    const done = this.done.has(this.lesson.id);
    this.shown.steps.querySelectorAll('button').forEach((step, i) => {
      step.setAttribute('aria-selected', String(i === this.index));
      step.classList.toggle('done', this.done.has(this.lessons[i].id));
    });
    this.shown.task.classList.toggle('done', done);
    this.buttons.next.classList.toggle('ready', done);
    if (done) this.shown.expected.hidden = true;
  }

  // A run of `lesson` ended as `outcome` says: the lesson is done if it did what it asks.
  judged(lesson: Lesson, outcome: Outcome) {
    if (lesson.expect === undefined) return;
    if (judge(lesson.expect, outcome)) {
      this.done.add(lesson.id);
      keep(DONE, JSON.stringify([...this.done]));
    } else if (lesson === this.lesson && !this.done.has(lesson.id)) {
      // Not yet: what the lesson wants, to compare the run with.
      const {expect} = lesson;
      const text = expect.output === undefined ? '' : tidy(expect.output);
      this.shown.expected.replaceChildren(wants(expect), ...(text === '' ? [] : [Object.assign(document.createElement('pre'), {textContent: text})]));
      this.shown.expected.hidden = false;
    }
    this.refresh();
  }
}
