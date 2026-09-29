// A session in a terminal: each command typed, then what it printed, as `pnpm record` captured it.

import {makeScene2D} from '@motion-canvas/2d';

import {codeScheme} from '../../brand/code';
import {useSchemeName} from '../../brand/palette';
import {Stage} from '../../clip/Stage';
import {Terminal} from '../../clip/Terminal';
import {Window} from '../../clip/Window';
import {beat, useSegment} from '../../clip/mode';

const HOLD = 2;

export default makeScene2D(function* (view) {
  const placed = useSegment('terminal');
  const stage = new Stage(view, placed);
  const {frame, area} = stage;
  const {session} = placed.segment;
  const {columns, rows} = Terminal.size(session);
  const terminal = new Terminal(codeScheme(useSchemeName()), session, Window.font(area, frame.side, columns, rows));
  terminal.window.root.position(area.center);
  view.add(terminal.window.root);

  yield* stage.enter();
  yield* terminal.window.open();
  yield* beat('run', 0);
  yield* terminal.play();
  yield* beat('terminal', HOLD);
});
