import '@fontsource-variable/jetbrains-mono';
import 'virtual:brand.css';
import '../style.css';
import '../play/workbench.css';
import '../play/play.css';
import './learn.css';

import lessons from 'virtual:lessons';

import {chrome} from '../chrome';
import {Keys} from '../keys';
import {encode} from '../play/share';
import {loadToolchain} from '../play/toolchain';
import {Workspace} from '../play/workspace';
import {Brief} from './brief';
import {Coach} from './coach';
import {Progress} from './progress';

// The course is the lessons that ask something; the free one is the playground's.
const course = lessons.filter((lesson) => lesson.expect !== undefined);
const progress = new Progress(course);

// A lesson named in the address, as `#/hello`, is the one shown.
const named = () => progress.find(location.hash.replace(/^#\//, ''));
if (named() >= 0) progress.go(named());

const find = <T extends HTMLElement>(selector: string) => document.querySelector(selector) as T;

const keys = new Keys();
const workspace = new Workspace(find('[data-workspace]'), loadToolchain(import.meta.env.BASE_URL), keys, {
  code: progress.code(),
  changed: (code) => progress.keepCode(code),
  ended: (outcome, code) => {
    coach.reset();
    brief.judged(progress.lesson, outcome, code);
  },
});
const {editor} = workspace;

const brief = new Brief(find('[data-brief]'), course, progress, (index) => go(index));
const coach = new Coach(brief.coach, find('[data-help]'), find('[data-help-label]'), find('[data-rungs]'), {
  code: () => editor.code,
  point: (lines) => editor.point(lines),
  replace: (code) => (editor.code = code),
});

// Shows the lesson `index`, with its code as it was left.
function go(index: number) {
  if (index < 0 || index >= course.length) return;
  progress.go(index);
  history.replaceState(null, '', `#/${progress.lesson.id}`);
  workspace.load(progress.code());
  coach.reset();
  brief.render(progress.lesson);
  linked();
}

// The playground link carries the code as it is when it is followed.
const open = find<HTMLAnchorElement>('[data-open]');
function linked() {
  open.href = `../play/#${encode(editor.code)}`;
}
open.addEventListener('pointerdown', linked);
open.addEventListener('focus', linked);

keys.on('hint', () => coach.next(progress.lesson));
keys.on('reset', () => {
  workspace.load(progress.lesson.start);
  coach.reset();
});
keys.on('playground', () => {
  linked();
  window.open(open.href, '_blank', 'noopener');
});
keys.on('previous-lesson', () => go(progress.index - 1));
keys.on('next-lesson', () => go(progress.index + 1));
keys.on('map', () => brief.toggleMap());
addEventListener('hashchange', () => named() >= 0 && named() !== progress.index && go(named()));

chrome();
history.replaceState(null, '', `#/${progress.lesson.id}`);
brief.render(progress.lesson);
coach.reset();
linked();
