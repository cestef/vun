// What a lesson asks, as a list of checks that fill in as runs go: each judged by the last run,
// the output asked for a click away, and once the lesson is done the reference solution beside what
// the learner wrote.

import MarkdownIt from 'markdown-it';
import {For, Show} from 'solid-js';

import type {IconName} from '../icons';
import {diff} from '../play/diff';
import {type Expect, type Outcome, type Standing, tidy, verdicts} from '../play/lessons';
import {DiffView} from '../ui/DiffView';
import {Icon} from '../ui/Icon';

const markdown = new MarkdownIt();
const MARKS: Record<Standing, IconName> = {pending: 'circle', met: 'check', missed: 'close'};

// The output asked for, a click away; once there is output that misses it, against what was printed.
function Expected(props: {wanted: string; outcome: Outcome | undefined}) {
  const printed = () => (props.outcome === undefined || !props.outcome.built ? undefined : tidy(props.outcome.output));
  const wrong = () => printed() !== undefined && printed() !== tidy(props.wanted);
  return (
    <details class="expected">
      <summary>
        <Icon name="chevron-right" />
        {wrong() ? 'Compare with the expected output' : 'Expected output'}
      </summary>
      <Show when={wrong()} fallback={<pre class="wanted">{tidy(props.wanted)}</pre>}>
        <DiffView changes={diff(printed()!, tidy(props.wanted))} />
      </Show>
    </details>
  );
}

export function Task(props: {task: string; expect: Expect; outcome: Outcome | undefined; done: boolean}) {
  const list = () => verdicts(props.expect, props.outcome);
  const met = () => list().filter((verdict) => verdict.standing === 'met').length;
  return (
    <section class={['task', {done: props.done}]}>
      <header>
        <Icon name="locate" />
        <h2>Your task</h2>
        <span class="tally">{`${met()} of ${list().length}`}</span>
      </header>
      <p class="goal" innerHTML={markdown.renderInline(props.task)}></p>
      <ul class="checks">
        <For each={list()}>
          {(verdict) => (
            <li class={['check', verdict.standing]}>
              <span class="mark">
                <Icon name={MARKS[verdict.standing]} />
              </span>
              <span class="name">
                <For each={verdict.name.split('`')}>{(part, i) => (i() % 2 === 1 ? <code>{part}</code> : part)}</For>
              </span>
              <Show when={verdict.cells}>
                {(cells) => (
                  <span class="strip">
                    <For each={cells()}>{(cell, i) => <i class={cell} title={`line ${i() + 1}`}></i>}</For>
                  </span>
                )}
              </Show>
              <Show when={verdict.found}>{(found) => <span class="found">{found()}</span>}</Show>
            </li>
          )}
        </For>
      </ul>
      <Show when={props.expect.output}>{(wanted) => <Expected wanted={wanted()} outcome={props.outcome} />}</Show>
    </section>
  );
}

// The lesson is done: the reference solution beside what the learner wrote, a click away.
export function Solved(props: {code: string; answer: string}) {
  const changes = () => diff(props.code, props.answer);
  const same = () => changes().every((change) => change.kind === 'same');
  return (
    <details class="compare">
      <summary>
        <Icon name="check" />
        {same() ? 'Done. Your code is the reference solution.' : 'Done. Compare with the reference solution.'}
      </summary>
      <Show when={!same()}>
        <DiffView changes={changes()} />
      </Show>
    </details>
  );
}
