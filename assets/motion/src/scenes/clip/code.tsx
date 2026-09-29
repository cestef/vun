// A program in a window: its lines written in one by one, then each range it focuses on picked
// out in turn.

import {makeScene2D} from '@motion-canvas/2d';

import {codeScheme} from '../../brand/code';
import {useSchemeName} from '../../brand/palette';
import {Program} from '../../clip/Program';
import {Stage} from '../../clip/Stage';
import {Window} from '../../clip/Window';
import {beat, useSegment} from '../../clip/mode';

const HOLD = 1.2;

export default makeScene2D(function* (view) {
  const placed = useSegment('code');
  const stage = new Stage(view, placed);
  const {frame, area} = stage;
  const scheme = useSchemeName();
  const {file, text, focus = []} = placed.segment.source;
  const {columns, rows} = Program.size([text]);
  const program = new Program(codeScheme(scheme), scheme, file, [text], Window.font(area, frame.side, columns, rows));
  program.window.root.position(area.center);
  view.add(program.window.root);

  yield* stage.enter();
  yield* program.window.open();
  yield* program.write();
  yield* program.focus(focus);
  yield* beat('code', HOLD);
});
