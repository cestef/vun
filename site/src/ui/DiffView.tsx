// A diff as it is shown: the changed lines, signed, with a line of the text around each.

import {For} from 'solid-js';

import type {Change} from '../play/diff';

// How many unchanged lines are shown around a change.
const CONTEXT = 1;
const SIGNS = {same: ' ', remove: '-', add: '+'};

function shown(changes: Change[]): Change[] {
  const near = new Set<number>();
  for (const [i, change] of changes.entries()) {
    if (change.kind === 'same') continue;
    for (let at = Math.max(0, i - CONTEXT); at <= Math.min(changes.length - 1, i + CONTEXT); at++) near.add(at);
  }
  return changes.filter((_, i) => near.has(i));
}

export function DiffView(props: {changes: Change[]}) {
  return (
    <div class="preview">
      <For each={shown(props.changes)}>{(change) => <span class={['row', change.kind]}>{`${SIGNS[change.kind]} ${change.text}`}</span>}</For>
    </div>
  );
}
