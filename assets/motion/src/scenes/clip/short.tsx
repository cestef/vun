// The sting, short, to open a clip: the echo slides in, the word is traced at its weight and
// lands; with the variable `mascot`, the mascot drops in beside it.

import {makeScene2D} from '@motion-canvas/2d';
import {all, chain, delay, easeInOutSine, easeOutCubic, useScene, waitFor} from '@motion-canvas/core';

import {Wordmark} from '../../brand/Wordmark';
import {SHORT} from '../../brand/beats';
import {LOGO} from '../../brand/logo';
import {Stage} from '../../clip/Stage';

// The share of the frame the word takes, how far the echo slides in from, a share of the
// shorter side, and how long the word holds at the end.
const SHARE = 0.55;
const SLIDE = 0.08;
const HOLD = 0.8;

export default makeScene2D(function* (view) {
  const {scheme, frame, echo} = new Stage(view);
  const mascot = useScene().variables.get('mascot', false)();
  const word = new Wordmark(scheme, frame.area.fit(LOGO.view, SHARE), {echo});
  word.erase();
  view.add(word.root);

  const at = echo.root.x();
  echo.root.x(at + frame.side * SLIDE);
  echo.root.opacity(0);
  yield* all(
    echo.root.opacity(1, 1, easeInOutSine),
    echo.root.x(at, 1.6, easeOutCubic),
    delay(SHORT.start, chain(word.draw(SHORT.pace), all(word.land(), ...(mascot ? [delay(SHORT.welcome, word.welcome())] : [])))),
  );
  yield* waitFor(HOLD);
});
