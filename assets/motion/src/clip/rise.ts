// Things in a layout coming in: each held below its place and hidden, then rising into it.

import type {Layout} from '@motion-canvas/2d';
import {type ThreadGenerator, all, easeOutCubic} from '@motion-canvas/core';

// How far a thing starts below its place, per unit of its font size, and how long it rises.
const DROP = 0.6;
const RISING = 0.5;

export function lower(nodes: Layout[]) {
  for (const node of nodes) {
    node.opacity(0);
    node.margin.top(node.fontSize() * DROP);
  }
}

export function* rise(node: Layout): ThreadGenerator {
  yield* all(node.opacity(1, RISING, easeOutCubic), node.margin.top(0, RISING, easeOutCubic));
}
