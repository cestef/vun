// A title and points under it, each coming in on its own beat beside a small v.

import {Layout, Txt, makeScene2D} from '@motion-canvas/2d';
import {all, easeOutCubic} from '@motion-canvas/core';

import {Mark} from '../../brand/Mark';
import {BOLD, FONT, SIZE} from '../../brand/type';
import {Stage} from '../../clip/Stage';
import {column} from '../../clip/column';
import {beat, useSegment} from '../../clip/mode';
import {lower, rise} from '../../clip/rise';

// How long each point holds before the next, and the last before the scene ends.
const POINT = 1.6;
const HOLD = 1.6;

export default makeScene2D(function* (view) {
  const placed = useSegment('points');
  const stage = new Stage(view, placed);
  const {scheme, frame, area} = stage;
  const size = (share: number) => frame.side * share;
  const {title, points} = placed.segment;

  const heading = (
    <Txt fontFamily={FONT} textWrap fontSize={size(SIZE.heading)} fontWeight={BOLD} fill={scheme.ink} text={title} />
  ) as Txt;
  const marks = points.map(() => Mark.boxed(scheme.accent, size(SIZE.body) * 0.8));
  const rows = points.map((point, i) => (
    <Layout layout alignItems="center" gap={size(SIZE.body) * 0.6} fontSize={size(SIZE.body)}>
      {marks[i].box}
      <Txt fontFamily={FONT} textWrap grow={1} fontSize={size(SIZE.body)} fill={scheme.text} text={point} />
    </Layout>
  )) as Layout[];
  for (const {mark} of marks) {
    mark.v.end(0);
  }
  view.add(column(area, size(SIZE.body), [heading, ...rows]));
  lower([heading, ...rows]);

  yield* stage.enter();
  yield* rise(heading);
  for (const [i, row] of rows.entries()) {
    yield* beat(`point ${i + 1}`, i === 0 ? 0.3 : POINT);
    yield* all(marks[i].mark.v.end(1, 0.4, easeOutCubic), rise(row));
  }
  yield* beat('points', HOLD);
});
