// A program beside the session that runs it, side by side or one above the other, whichever lets
// their type be larger, both in one size of type. The program is written in, the session plays, then the
// program's focus is picked out.

import {makeScene2D} from '@motion-canvas/2d';
import {all, delay} from '@motion-canvas/core';

import {codeScheme} from '../../brand/code';
import {useSchemeName} from '../../brand/palette';
import {Program} from '../../clip/Program';
import {Stage} from '../../clip/Stage';
import {Terminal} from '../../clip/Terminal';
import {Window} from '../../clip/Window';
import {beat, useSegment} from '../../clip/mode';

// The gap between the two windows, a share of the shorter side.
const GAP = 0.04;
const HOLD = 2;

export default makeScene2D(function* (view) {
  const placed = useSegment('split');
  const stage = new Stage(view, placed);
  const {frame, area} = stage;
  const scheme = useSchemeName();
  const colors = codeScheme(scheme);
  const {source, session} = placed.segment;
  const shown = Program.size([source.text]);
  const ran = Terminal.size(session);
  const arranged = [true, false].map((across) => {
    const [left, right] = area.halves(frame.side * GAP, across);
    const font = Math.min(
      Window.font(left, frame.side, shown.columns, shown.rows),
      Window.font(right, frame.side, ran.columns, ran.rows),
    );
    return {left, right, font};
  });
  const {left, right, font} = arranged.reduce((best, next) => (next.font > best.font ? next : best));
  const program = new Program(colors, scheme, source.file, [source.text], font);
  const terminal = new Terminal(colors, session, font);
  program.window.root.position(left.center);
  terminal.window.root.position(right.center);
  view.add([program.window.root, terminal.window.root]);

  yield* stage.enter();
  yield* all(program.window.open(), delay(0.15, terminal.window.open()));
  yield* program.write();
  yield* beat('run', 0.4);
  yield* terminal.play();
  yield* program.focus(source.focus ?? []);
  yield* beat('split', HOLD);
});
