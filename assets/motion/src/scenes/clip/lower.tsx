// Who speaks, over footage: in the bottom left corner, a panel comes in, the v is traced beside
// the name, the name and role typed; they hold, then go as they came. Drawn over no background,
// for the footage to show through.

import {Layout, Rect, Txt, makeScene2D} from '@motion-canvas/2d';
import {all, delay, easeInCubic, easeOutCubic, waitFor} from '@motion-canvas/core';

import {Mark} from '../../brand/Mark';
import {useScheme} from '../../brand/palette';
import {FONT} from '../../brand/type';
import {Frame} from '../../clip/frame';
import {useEpisode} from '../../clip/mode';

// Sizes, shares of the shorter side: the name, the role, the panel's padding and radius, and the
// room between the v and the text; how far the panel slides in, per unit of its padding; seconds
// per character typed and how long it holds.
const SIZE = {name: 0.04, role: 0.028, padding: 0.024, radius: 0.016, gap: 0.02};
const SLIDE = 2;
const TYPING = 0.03;
const HOLD = 3;

export default makeScene2D(function* (view) {
  const scheme = useScheme();
  const frame = new Frame(view.size());
  const {name, role} = useEpisode().speaker;
  const size = (share: number) => frame.side * share;

  const text = {fontFamily: FONT, text: ''} as const;
  const lines = [
    <Txt {...text} fontSize={size(SIZE.name)} fontWeight={700} fill={scheme.ink} />,
    <Txt {...text} fontSize={size(SIZE.role)} fill={scheme.muted} />,
  ] as Txt[];
  // The v as tall as the two lines.
  const tall = size(SIZE.name + SIZE.role) * 1.3;
  const {mark, box} = Mark.boxed(scheme.accent, tall);
  mark.v.end(0);
  const panel = (
    <Rect
      layout
      alignItems="center"
      gap={size(SIZE.gap)}
      padding={size(SIZE.padding)}
      radius={size(SIZE.radius)}
      fill={scheme.background}
      offset={[-1, 1]}
      position={[frame.area.start.x, -frame.area.start.y]}
      opacity={0}
    >
      {box}
      <Layout layout direction="column">
        {...lines}
      </Layout>
    </Rect>
  ) as Rect;
  view.add(panel);

  const slide = size(SIZE.padding) * SLIDE;
  const at = panel.x();
  const typed = [name, role];
  panel.x(at - slide);
  yield* all(
    panel.opacity(1, 0.4, easeOutCubic),
    panel.x(at, 0.5, easeOutCubic),
    delay(0.2, mark.v.end(1, 0.5, easeOutCubic)),
    delay(0.4, all(...lines.map((line, i) => line.text(typed[i], typed[i].length * TYPING)))),
  );
  yield* waitFor(HOLD);
  yield* all(
    ...lines.map((line) => line.text('', 0.3)),
    mark.v.end(0, 0.4, easeInCubic),
    delay(0.2, all(panel.opacity(0, 0.4, easeInCubic), panel.x(at - slide, 0.4, easeInCubic))),
  );
});
