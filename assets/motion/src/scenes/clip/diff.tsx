// A change to a program: its text before written in, then changed into the text after, what
// stays moving into its new place.

import {makeScene2D} from '@motion-canvas/2d';

import {codeScheme} from '../../brand/code';
import {useSchemeName} from '../../brand/palette';
import {Program} from '../../clip/Program';
import {Stage} from '../../clip/Stage';
import {Window} from '../../clip/Window';
import {beat, useSegment} from '../../clip/mode';

const BEFORE = 1.4;
const HOLD = 2;

export default makeScene2D(function* (view) {
  const placed = useSegment('diff');
  const stage = new Stage(view, placed);
  const {frame, area} = stage;
  const scheme = useSchemeName();
  const {file, before, after} = placed.segment;
  const texts = [before, after];
  const {columns, rows} = Program.size(texts);
  const program = new Program(codeScheme(scheme), scheme, file, texts, Window.font(area, frame.side, columns, rows));
  program.window.root.position(area.center);
  view.add(program.window.root);

  yield* stage.enter();
  yield* program.window.open();
  yield* program.write();
  yield* beat('before', BEFORE);
  yield* program.change(1);
  yield* beat('after', HOLD);
});
