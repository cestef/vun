// One sentence set large beside an accent rule, where it comes from under it.

import {Layout, Rect, Txt, makeScene2D} from '@motion-canvas/2d';
import {all, delay, easeOutCubic} from '@motion-canvas/core';

import {FONT, SIZE} from '../../brand/type';
import {Stage} from '../../clip/Stage';
import {beat, useSegment} from '../../clip/mode';
import {lower, rise} from '../../clip/rise';

// The rule's width and the room after it, shares of the shorter side; seconds held per word.
const RULE = 0.008;
const ROOM = 0.035;
const READING = 0.3;
// The share of the area's width the sentence wraps at.
const MEASURE = 0.85;

export default makeScene2D(function* (view) {
  const placed = useSegment('callout');
  const stage = new Stage(view, placed);
  const {scheme, frame, area} = stage;
  const size = (share: number) => frame.side * share;
  const {text, source} = placed.segment;

  const rule = (<Rect width={size(RULE)} radius={size(RULE) / 2} fill={scheme.accent} scale={[1, 0]} />) as Rect;
  const lines = [
    <Txt fontFamily={FONT} textWrap fontSize={size(SIZE.heading)} lineHeight="125%" fill={scheme.ink} text={text} />,
    ...(source === undefined ? [] : [<Txt fontFamily={FONT} fontSize={size(SIZE.small)} fill={scheme.muted} text={source} />]),
  ] as Txt[];
  view.add(
    <Layout layout gap={size(ROOM)} width={area.size.x * MEASURE} offset={[-1, 0]} position={[area.start.x, area.center.y]}>
      {rule}
      <Layout layout direction="column" gap={size(SIZE.small)} grow={1}>
        {lines}
      </Layout>
    </Layout>,
  );
  lower(lines);

  yield* stage.enter();
  yield* all(rule.scale([1, 1], 0.5, easeOutCubic), delay(0.2, all(...lines.map((line, i) => delay(0.2 * i, rise(line))))));
  yield* beat('callout', text.split(' ').length * READING);
});
