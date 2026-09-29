// Numbers counted up side by side, what each counts under it: in a row in a wide frame, a column
// in a tall one.

import {Layout, Txt, makeScene2D} from '@motion-canvas/2d';
import {createSignal, easeOutCubic, sequence} from '@motion-canvas/core';

import {BOLD, FONT, SIZE} from '../../brand/type';
import {Stage} from '../../clip/Stage';
import {beat, useSegment} from '../../clip/mode';
import {lower, rise} from '../../clip/rise';

const COUNTING = 1.2;
const HOLD = 2;

export default makeScene2D(function* (view) {
  const placed = useSegment('stats');
  const stage = new Stage(view, placed);
  const {scheme, frame, area} = stage;
  const size = (share: number) => frame.side * share;
  const {stats} = placed.segment;
  const wide = area.size.x > area.size.y;

  const counts = stats.map(() => createSignal(0));
  const cells = stats.map(({label}, i) => (
    <Layout layout direction="column" alignItems="start" gap={size(SIZE.small) * 0.4} fontSize={size(SIZE.small)}>
      <Txt
        fontFamily={FONT}
        fontSize={size(SIZE.display)}
        fontWeight={BOLD}
        fill={scheme.accent}
        text={() => Math.round(counts[i]()).toLocaleString('en')}
      />
      <Txt fontFamily={FONT} fontSize={size(SIZE.small)} fill={scheme.text} text={label} />
    </Layout>
  )) as Layout[];
  view.add(
    <Layout
      layout
      direction={wide ? 'row' : 'column'}
      justifyContent="space-between"
      gap={size(SIZE.body)}
      width={wide ? area.size.x : undefined}
      position={area.center}
    >
      {cells}
    </Layout>,
  );
  lower(cells);

  yield* stage.enter();
  yield* sequence(0.2, ...cells.map((cell, i) => sequence(0, rise(cell), counts[i](stats[i].value, COUNTING, easeOutCubic))));
  yield* beat('stats', HOLD);
});
