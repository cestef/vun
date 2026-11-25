// The problems of the last build: a card for each diagnostic, in the order the compiler said them. A
// card and the spans it owns light each other as the pointer reaches either, and a click on a
// span's note brings the card forward.

import MarkdownIt from 'markdown-it';
import {For, Show, createMemo} from 'solid-js';

import {DiffView} from '../ui/DiffView';
import {Icon} from '../ui/Icon';
import {Message} from '../ui/Message';
import type {Bench} from './bench';
import {diff} from './diff';
import {type Entry, explanation} from './entry';
import {Excerpt} from './Excerpt';
import {coloredBlocks} from './highlight';
import type {At, Fix} from './protocol';
import {createSignal} from 'solid-js';

const markdown = new MarkdownIt();

function Place(props: {bench: Bench; at: At; reveal: () => void}) {
  const shown = () => `${props.at.file}:${props.at.start.line}:${props.at.start.column}`;
  return (
    <Show when={props.bench.holds(props.at)} fallback={<p class="place">{shown()}</p>}>
      <p>
        <button type="button" class="place" onClick={props.reveal}>
          {shown()}
        </button>
      </p>
    </Show>
  );
}

// A fix: what it says and an apply button, with its change shown under it first.
function FixView(props: {bench: Bench; fix: Fix}) {
  const {bench} = props;
  const applied = () => bench.applied.value.has(props.fix);
  const stale = () => bench.stale.value.has(props.fix);
  const changes = createMemo(() => diff(bench.text, bench.patched(props.fix)));
  return (
    <div class="fixing">
      <button
        type="button"
        class={['fix', {applied: applied(), stale: stale()}]}
        title={stale() ? 'the code changed since this build' : undefined}
        disabled={bench.closed.value && !applied()}
        onClick={() => bench.apply(props.fix)}
      >
        <span class="apply">
          <Show when={applied()} fallback={<><Icon name="wand" />Apply</>}>
            <Icon name="check" />Applied
          </Show>
        </span>
        <Message message={props.fix.message} />
        <Show when={props.fix.safety !== 'safe'}>
          <span class={`safety ${props.fix.safety}`}>{props.fix.safety}</span>
        </Show>
      </button>
      <DiffView changes={changes()} />
    </div>
  );
}

function Problem(props: {bench: Bench; entry: Entry}) {
  const {bench} = props;
  const d = () => props.entry.diagnostic;
  const [open, setOpen] = createSignal(false);
  const page = () => explanation(bench.library.value, d().path);
  const labels = () => [...(d().primary ? [d().primary!] : []), ...d().labels].filter((label) => bench.holds(label.at));
  return (
    <article
      class={['problem', d().severity, {focused: bench.focused.value === props.entry.index}]}
      data-problem={props.entry.index}
      onMouseEnter={() => bench.light(props.entry.index)}
      onMouseLeave={() => bench.light(undefined)}
    >
      <header>
        <button type="button" class="head" title="show in the editor" onClick={() => bench.reveal(d())}>
          <Icon name={d().severity === 'error' ? 'alert' : 'warning'} />
          <span class="message">
            <Message message={d().message} />
          </span>
        </button>
        <button type="button" class={['id', {open: open()}]} disabled={page() === undefined} onClick={() => setOpen(!open())}>
          {d().id}
        </button>
      </header>
      <Show when={d().primary?.at}>{(at) => <Place bench={bench} at={at()} reveal={() => bench.reveal(d())} />}</Show>
      <Show when={labels().length > 0}>
        <Excerpt source={props.entry.source} labels={labels()} severity={d().severity} jump={(label) => bench.select(label.at)} />
      </Show>
      <For each={d().children}>
        {(child) => (
          <>
            <p class={`child ${child.kind}`}>
              <b>{child.kind}</b>: <Message message={child.message} />
            </p>
            <For each={child.fixes}>{(fix) => <FixView bench={bench} fix={fix} />}</For>
          </>
        )}
      </For>
      <For each={d().fixes}>{(fix) => <FixView bench={bench} fix={fix} />}</For>
      <Show when={open() && page()}>{(text) => <div class="explanation" innerHTML={coloredBlocks(markdown.render(text()))} />}</Show>
    </article>
  );
}

export function Problems(props: {bench: Bench}) {
  return (
    <div class="problems">
      <For each={props.bench.entries.value} fallback={<p class="none">{props.bench.built.value ? 'No problems.' : 'Nothing to report yet.'}</p>}>
        {(entry) => <Problem bench={props.bench} entry={entry} />}
      </For>
    </div>
  );
}
