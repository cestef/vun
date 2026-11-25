// The lines a diagnostic's labels point into: each range marked and clickable, each label's
// message under its line, with the way to a label on another line.

import {For, Show} from 'solid-js';

import {Message} from '../ui/Message';
import type {Label} from './protocol';
import type {Source} from './source';

// How many lines around its labels an excerpt shows at most.
const EXCERPT = 8;

interface Mark {
  label: Label;
  primary: boolean;
  from: number;
  to: number;
}

// The marks of `labels` on line `number`, in the order of the text and not overlapping.
function marks(source: Source, labels: Label[], number: number): Mark[] {
  const text = source.lines()[number - 1] ?? '';
  const start = source.start(number);
  let end = 0;
  return labels
    .map((label, i) => ({label, primary: i === 0, from: source.from(label.at) - start, to: source.to(label.at) - start}))
    .filter((m) => m.to >= 0 && m.from <= text.length)
    .map((m) => ({...m, from: Math.max(m.from, 0), to: Math.min(Math.max(m.to, m.from + 1), text.length + 1)}))
    .sort((a, b) => a.from - b.from)
    .filter((m) => m.from >= end && ((end = m.to), true));
}

export function Excerpt(props: {source: Source; labels: Label[]; severity: string; jump: (label: Label) => void}) {
  const first = () => Math.min(...props.labels.map((l) => l.at.start.line));
  const last = () => Math.min(Math.max(...props.labels.map((l) => l.at.end.line)), first() + EXCERPT - 1);
  const numbers = () => Array.from({length: last() - first() + 1}, (_, i) => first() + i);
  const far = () => props.labels.filter((l) => l.at.start.line > last() || l.at.end.line < first());
  return (
    <div class="excerpt">
      <For each={numbers()}>
        {(number) => {
          const text = () => props.source.lines()[number - 1] ?? '';
          const line = () => marks(props.source, props.labels, number);
          const pieces = () => {
            const out: {text: string; mark?: Mark}[] = [];
            let at = 0;
            for (const mark of line()) {
              out.push({text: text().slice(at, mark.from)}, {text: text().slice(mark.from, mark.to) || ' ', mark});
              at = mark.to;
            }
            out.push({text: text().slice(at)});
            return out;
          };
          return (
            <>
              <span class="number">{number}</span>
              <span class="code">
                <For each={pieces()}>
                  {(piece) => (
                    <Show when={piece.mark} fallback={piece.text}>
                      {(mark) => (
                        <button type="button" class={mark().primary ? `range primary ${props.severity}` : 'range label'} title="show in the editor" onClick={() => props.jump(mark().label)}>
                          {piece.text}
                        </button>
                      )}
                    </Show>
                  )}
                </For>
              </span>
              <For each={line().filter((m) => m.label.message !== null && m.label.at.end.line === number)}>
                {(mark) => (
                  <>
                    <span class="number"></span>
                    <span class={mark.primary ? `note primary ${props.severity}` : 'note label'} style={{'padding-left': `${mark.from}ch`}}>
                      <Message message={mark.label.message!} />
                    </span>
                  </>
                )}
              </For>
            </>
          );
        }}
      </For>
      <For each={far()}>
        {(label) => (
          <>
            <span class="number"></span>
            <button type="button" class="far" onClick={() => props.jump(label)}>
              {`line ${label.at.start.line}`}
              <Show when={label.message}>{(message) => <>: <Message message={message()} /></>}</Show>
            </button>
          </>
        )}
      </For>
    </div>
  );
}
