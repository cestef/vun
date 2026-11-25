// A lesson's editor under its prose: the task and its checks, the workbench, and the help. A run is
// judged here; a solved lesson says so, and is marked done in the sidebar.

import {Show, createMemo} from 'solid-js';

import {Bench} from '../play/bench';
import './lesson.css';
import {type Expect, type Outcome, judge} from '../play/lessons';
import {encode} from '../play/share';
import {loadToolchain} from '../play/toolchain';
import {Workbench} from '../play/Workbench';
import {href} from '../site';
import {Cell} from '../ui/cell';
import {Keys} from '../ui/keys';
import {Icon} from '../ui/Icon';
import {Action} from '../ui/Keycaps';
import {Coach, Ladder} from './Coach';
import {Progress} from './progress';
import {Solved, Task} from './Task';

export interface Material {
  id: string;
  task: string;
  hint: string;
  expect: Expect | undefined;
  start: string;
  answer: string;
}

const progress = new Progress();

export function Lesson(props: {lesson: Material}) {
  const {lesson} = props;
  const keys = new Keys();
  const outcome = new Cell<Outcome | undefined>(undefined);
  const solved = new Cell<string | undefined>(undefined);
  const bench = new Bench(keys, loadToolchain(href('')), {
    code: progress.code(lesson.id, lesson.start),
    changed: (code) => progress.keepCode(lesson.id, code),
    ended: (ended, code) => {
      outcome.value = ended;
      ladder.reset();
      if (lesson.expect !== undefined && judge(lesson.expect, ended)) {
        progress.finish(lesson.id);
        solved.value = code;
      }
    },
  });
  const ladder = new Ladder(lesson, bench);
  const done = createMemo(() => progress.done.value.has(lesson.id));
  const link = () => `${href('play/')}#${encode(bench.editor.code)}`;
  keys.on('hint', () => ladder.next());
  keys.on('reset', () => {
    bench.load(lesson.start);
    outcome.value = undefined;
    solved.value = undefined;
    ladder.reset();
  });
  keys.on('playground', () => window.open(link(), '_blank', 'noopener'));
  return (
    <div class="lesson not-content">
      <Show when={lesson.expect}>
        {(expect) => <Task task={lesson.task} expect={expect()} outcome={outcome.value} done={done()} />}
      </Show>
      <Workbench
        bench={bench}
        tools={
          <>
            <Action keys={keys} action="reset" class="tool" icon="reset">
              Reset
            </Action>
            <a class="tool" href={href('play/')} target="_blank" rel="noopener" onPointerDown={(event) => (event.currentTarget.href = link())} onFocus={(event) => (event.currentTarget.href = link())}>
              <Icon name="external" />
              Open in the playground
            </a>
          </>
        }
      />
      <Show when={lesson.expect}>
        <Coach ladder={ladder} bench={bench} />
        <Show when={solved.value}>{(code) => <Solved code={code()} answer={lesson.answer} />}</Show>
      </Show>
    </div>
  );
}
