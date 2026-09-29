// The end of a clip: the wordmark with the mascot beside it, which hops, and where to find Vun
// typed under them.

import {Txt, makeScene2D} from '@motion-canvas/2d';
import {all, delay} from '@motion-canvas/core';

import {Wordmark} from '../../brand/Wordmark';
import {LOGO} from '../../brand/logo';
import {FONT} from '../../brand/type';
import {Stage} from '../../clip/Stage';
import {beat, useEpisode, useSegment} from '../../clip/mode';

// The share of the frame the word takes, how far it sits above the middle and the address below
// it, and the address's size, shares of the shorter side; how high the mascot hops, per unit of
// its scale, and how long the card holds.
const SHARE = 0.5;
const ABOVE = 0.06;
const BELOW = 0.26;
const SIZE = 0.036;
const HOP = 40;
const HOLD = 2.5;

export default makeScene2D(function* (view) {
  const stage = new Stage(view, useSegment('end'));
  const {scheme, frame, area, echo} = stage;
  const scale = area.fit(LOGO.view, SHARE);
  const word = new Wordmark(scheme, scale, {echo});
  word.root.y(-frame.side * ABOVE);
  const mascot = word.beside('calm');
  word.roomy();
  const url = (
    <Txt y={frame.side * BELOW} fontFamily={FONT} fontSize={frame.side * SIZE} fill={scheme.muted} text="" />
  ) as Txt;
  view.add([word.root, url]);

  const address = useEpisode().url;
  yield* stage.enter();
  yield* all(
    word.land(0.5),
    delay(0.2, all(mascot.feel('glee'), mascot.hop(HOP * scale))),
    delay(0.5, url.text(address, address.length * 0.04)),
  );
  yield* delay(0.6, mascot.blink());
  yield* beat('end', HOLD);
});
