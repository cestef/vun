// The title of an episode over the echo, the cover of a talk: its series and number typed in, its
// title and who speaks rising after; the echo sways as the title lands.

import {Txt, makeScene2D} from '@motion-canvas/2d';
import {all, delay, sequence} from '@motion-canvas/core';

import {BOLD, FONT, SIZE} from '../../brand/type';
import {Stage} from '../../clip/Stage';
import {column} from '../../clip/column';
import {beat, useEpisode, useSegment} from '../../clip/mode';
import {lower, rise} from '../../clip/rise';

const HOLD = 2.4;

export default makeScene2D(function* (view) {
  const placed = useSegment('title');
  const stage = new Stage(view, placed);
  const {scheme, frame, area, echo} = stage;
  const {series, number, title, speaker} = useEpisode();
  const size = (share: number) => frame.side * share;
  const text = {fontFamily: FONT, textWrap: true} as const;

  const label = (<Txt {...text} fontSize={size(SIZE.small)} fill={scheme.accent} text="" />) as Txt;
  const lines = [
    <Txt {...text} fontSize={size(SIZE.display)} fontWeight={BOLD} lineHeight="115%" fill={scheme.ink} text={title} />,
    <Txt {...text} fontSize={size(SIZE.small)} fill={scheme.muted}>
      <Txt fill={scheme.text} text={speaker.name} />
      {`, ${speaker.role}`}
    </Txt>,
  ] as Txt[];
  view.add(column(area, size(SIZE.small), [label, ...lines]));
  lower(lines);

  yield* stage.enter();
  yield* label.text(`${series} ${String(number).padStart(2, '0')}`, 0.5);
  yield* all(sequence(0.15, ...lines.map(rise)), delay(0.3, echo.answer()));
  yield* beat('title', HOLD);
});
