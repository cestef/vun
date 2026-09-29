// A chapter begins: its number among the episode's chapters typed in, its title rising after, the
// echo swaying as it lands.

import {Txt, makeScene2D} from '@motion-canvas/2d';
import {all, delay} from '@motion-canvas/core';

import {BOLD, FONT, SIZE} from '../../brand/type';
import {Stage} from '../../clip/Stage';
import {column} from '../../clip/column';
import {beat, useEpisode, useSegment} from '../../clip/mode';
import {lower, rise} from '../../clip/rise';

const HOLD = 1.6;

export default makeScene2D(function* (view) {
  const placed = useSegment('chapter');
  const stage = new Stage(view, placed);
  const {scheme, frame, area, echo} = stage;
  const size = (share: number) => frame.side * share;
  const ordinal = useEpisode()
    .segments.slice(0, placed.index + 1)
    .filter((segment) => segment.kind === 'chapter').length;

  const label = (<Txt fontFamily={FONT} fontSize={size(SIZE.small)} fill={scheme.accent} text="" />) as Txt;
  const title = (
    <Txt fontFamily={FONT} textWrap fontSize={size(SIZE.display)} fontWeight={BOLD} lineHeight="115%" fill={scheme.ink} text={placed.segment.title} />
  ) as Txt;
  view.add(column(area, size(SIZE.small), [label, title]));
  lower([title]);

  yield* stage.enter();
  yield* label.text(String(ordinal).padStart(2, '0'), 0.3);
  yield* all(rise(title), delay(0.3, echo.answer()));
  yield* beat('chapter', HOLD);
});
