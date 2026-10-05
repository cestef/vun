// A diff as it is shown: the changed lines, signed, with a line of the text around each.

import type {Change} from './diff';
import {element} from './dom';

// How many unchanged lines are shown around a change.
const CONTEXT = 1;

export function diffView(changes: Change[]): HTMLElement {
  const shown = new Set<number>();
  for (const [i, change] of changes.entries()) {
    if (change.kind === 'same') continue;
    for (let near = Math.max(0, i - CONTEXT); near <= Math.min(changes.length - 1, i + CONTEXT); near++) shown.add(near);
  }
  const rows = element('div', 'preview');
  for (const [i, change] of changes.entries()) {
    if (!shown.has(i)) continue;
    const sign = {same: ' ', remove: '-', add: '+'}[change.kind];
    rows.append(element('span', `row ${change.kind}`, `${sign} ${change.text}`));
  }
  return rows;
}
