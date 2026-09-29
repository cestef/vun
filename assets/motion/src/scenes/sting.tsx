// The logo built as it is drawn in `construction.svg`: the grid opens from the middle, the
// guides and circles are traced, the joints set, the strokes drawn as hairlines by a pen, then
// swelled to their weight; the word lands, the v wags on its tip and the echo behind sways. With
// the variable `mascot`, the word then makes room and the mascot drops in beside it.

import {Circle, Node, Path, makeScene2D} from '@motion-canvas/2d';
import {
  Vector2,
  all,
  createRef,
  createRefArray,
  delay,
  easeInCubic,
  easeInOutCubic,
  easeInOutSine,
  easeOutBack,
  easeOutCubic,
  sequence,
  useScene,
  waitFor,
} from '@motion-canvas/core';

import {Mark} from '../brand/Mark';
import {Wordmark} from '../brand/Wordmark';
import {STING} from '../brand/beats';
import {LOGO} from '../brand/logo';
import {useScheme} from '../brand/palette';

// Pixels per unit of the logo's view box.
const SCALE = 3;
// The echo of the v behind, right of the word and cut by the right and bottom edges: pixels per
// unit, and where the origin of its view box sits.
const ECHO = {scale: 6, at: new Vector2(480, -190)};
// How long the grid takes to open from the middle to its edges.
const OPENING = 0.7;
// How long the word holds still at the end.
const HOLD = 1.2;

export default makeScene2D(function* (view) {
  const scheme = useScheme();
  const mascot = useScene().variables.get('mascot', false)();
  view.fill(scheme.background);

  const {view: box, construction} = LOGO;
  const middle = new Vector2(box.width / 2, box.height / 2);

  const camera = createRef<Node>();
  const grid = createRefArray<Path>();
  const guides = createRefArray<Path>();
  const circles = createRefArray<Circle>();
  const joints = createRefArray<Circle>();

  const echo = new Mark(scheme.echo, ECHO.scale);
  echo.root.position(ECHO.at.addX(80));
  echo.root.opacity(0);
  const word = new Wordmark(scheme, SCALE, {lineWidth: construction.hairline, echo});
  word.erase();

  const dots = (list: typeof construction.joints, fill: string) =>
    list.map(({x, y, r}) => <Circle ref={joints} x={x} y={y} size={3 * r} fill={fill} scale={0} />);
  word.canvas.insert(
    [
      ...construction.grid.map((data) => <Path ref={grid} data={data} stroke={scheme.echo} lineWidth={0.5} opacity={0} />),
      ...construction.guides.map((data) => (
        <Path ref={guides} data={data} stroke={scheme.muted} lineWidth={0.75} lineDash={[3, 3]} start={0.5} end={0.5} />
      )),
      ...construction.circles.map(({x, y, r}) => (
        <Circle ref={circles} x={x} y={y} size={2 * r} stroke={scheme.muted} lineWidth={0.75} lineDash={[2, 2]} rotation={-90} end={0} />
      )),
    ],
    0,
  );
  word.canvas.add([...dots(construction.joints, scheme.ink), ...dots(construction.centers, scheme.accent)]);

  view.add(
    <Node ref={camera} scale={1.06}>
      {echo.root}
      {word.root}
    </Node>,
  );

  const reach = Math.hypot(middle.x, middle.y);
  const away = (at: Vector2) => (at.sub(middle).magnitude / reach) * OPENING;

  yield* all(
    camera().scale(1, 6, easeOutCubic),
    echo.root.opacity(1, 2.5, easeInOutSine),
    echo.root.position(ECHO.at, 4, easeOutCubic),
    all(...grid.map((line) => delay(away(line.getPointAtPercentage(0.5).position), line.opacity(1, 0.35)))),
    delay(
      0.3,
      all(
        ...guides.map((guide) => all(guide.start(0, 0.7, easeInOutCubic), guide.end(1, 0.7, easeInOutCubic))),
        sequence(0.06, ...circles.map((circle) => circle.end(1, 0.6, easeInOutCubic))),
        delay(0.3, sequence(0.035, ...joints.map((joint) => joint.scale(1, 0.35, easeOutBack)))),
      ),
    ),
    delay(STING.build, build()),
  );

  function* build() {
    yield* word.draw();
    yield* all(
      word.swell(),
      ...joints.map((joint) => joint.scale(0, 0.25, easeInCubic)),
      ...guides.map((guide) => guide.opacity(0, 0.3)),
      ...circles.map((circle) => circle.opacity(0, 0.3)),
      ...grid.map((line) => line.opacity(0, 0.6)),
      delay(STING.land, word.land()),
      // While the echo still sways.
      ...(mascot ? [delay(STING.welcome, word.welcome())] : []),
    );
  }

  yield* waitFor(HOLD);
});
