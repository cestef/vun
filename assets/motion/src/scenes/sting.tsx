// The logo built as it is drawn in `construction.svg`: the grid opens from the middle, the
// guides and circles are traced, the joints set, the strokes drawn as hairlines by a pen, then
// swelled to their weight; the word lands, the v wags on its tip and the echo behind sways. With
// the variable `mascot`, the word then makes room and the mascot drops in beside it.

import { Circle, Node, Path, makeScene2D } from '@motion-canvas/2d';
import {
  Vector2,
  all,
  chain,
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

import { LOGO } from '../brand/logo';
import { scheme as named } from '../brand/palette';
import { Mascot } from '../mascot/Mascot';
import { extent } from '../mascot/svg';

// Pixels per unit of the logo's view box.
const SCALE = 3;
// The echo of the v behind, right of the word and cut by the right and bottom edges: pixels per
// unit, and where the origin of its view box sits.
const ECHO = { scale: 6, at: new Vector2(480, -190) };
// The pen's dot, in units of the view box.
const PEN = 7;
// How long the grid takes to open from the middle to its edges.
const OPENING = 0.7;
// The mascot drops from this far above its place, and the word and the mascot together take
// this much of the word's size alone, to fit.
const DROP = 1100;
const ROOM = 0.86;
// How long the word holds still at the end.
const HOLD = 1.2;

export default makeScene2D(function*(view) {
  const variables = useScene().variables;
  const scheme = named(variables.get('scheme', 'dark')());
  view.fill(scheme.background);

  const { view: box, weight, baseline, construction } = LOGO;
  const middle = new Vector2(box.width / 2, box.height / 2);

  const camera = createRef<Node>();
  const room = createRef<Node>();
  const lockup = createRef<Node>();
  const echo = createRef<Node>();
  const sway = createRef<Node>();
  const word = createRef<Node>();
  const tip = createRef<Node>();
  const v = createRef<Path>();
  const un = createRef<Path>();
  const pens = createRefArray<Circle>();
  const grid = createRefArray<Path>();
  const guides = createRefArray<Path>();
  const circles = createRefArray<Circle>();
  const joints = createRefArray<Circle>();

  const stroke = { lineCap: 'round', lineJoin: 'round' } as const;
  const dots = (list: typeof construction.joints, fill: string) =>
    list.map(({ x, y, r }) => <Circle ref={joints} x={x} y={y} size={3 * r} fill={fill} scale={0} />);

  view.add(
    <Node ref={camera} scale={1.06}>
      <Node ref={echo} position={ECHO.at.addX(80)} scale={ECHO.scale} opacity={0}>
        <Node ref={sway}>
          <Node>
            <Path data={LOGO.v} stroke={scheme.echo} lineWidth={weight} {...stroke} />
          </Node>
        </Node>
      </Node>
      <Node ref={room}>
        <Node ref={lockup} y={(baseline - middle.y) * SCALE}>
          <Node ref={word}>
            <Node scale={SCALE} x={-middle.x * SCALE} y={-baseline * SCALE}>
              {construction.grid.map((data) => (
                <Path ref={grid} data={data} stroke={scheme.echo} lineWidth={0.5} opacity={0} />
              ))}
              {construction.guides.map((data) => (
                <Path ref={guides} data={data} stroke={scheme.muted} lineWidth={0.75} lineDash={[3, 3]} start={0.5} end={0.5} />
              ))}
              {construction.circles.map(({ x, y, r }) => (
                <Circle ref={circles} x={x} y={y} size={2 * r} stroke={scheme.muted} lineWidth={0.75} lineDash={[2, 2]} rotation={-90} end={0} />
              ))}
              <Node ref={tip}>
                <Node>
                  <Path ref={v} data={LOGO.v} stroke={scheme.accent} lineWidth={construction.hairline} end={0} {...stroke} />
                  <Circle ref={pens} size={PEN} fill={scheme.accent} scale={0} />
                </Node>
              </Node>
              <Path ref={un} data={LOGO.un} stroke={scheme.ink} lineWidth={construction.hairline} end={0} {...stroke} />
              <Circle ref={pens} size={PEN} fill={scheme.ink} scale={0} />
              {dots(construction.joints, scheme.ink)}
              {dots(construction.centers, scheme.accent)}
            </Node>
          </Node>
        </Node>
      </Node>
    </Node>,
  );

  // Both v turn on their tip: the pivot sits there, its child is moved back by as much.
  const point = v().getPointAtPercentage(0.5).position;
  for (const pivot of [tip(), sway()]) {
    pivot.position(point);
    pivot.children()[0].position(point.scale(-1));
  }

  for (const [pen, path] of [
    [pens[0], v()],
    [pens[1], un()],
  ] as const) {
    pen.position(() => path.getPointAtPercentage(path.end()).position);
  }

  const reach = Math.hypot(middle.x, middle.y);
  const away = (at: Vector2) => (at.sub(middle).magnitude / reach) * OPENING;

  yield* all(
    camera().scale(1, 6, easeOutCubic),
    echo().opacity(1, 2.5, easeInOutSine),
    echo().position(ECHO.at, 4, easeOutCubic),
    all(...grid.map((line) => delay(away(line.getPointAtPercentage(0.5).position), line.opacity(1, 0.35)))),
    delay(
      0.3,
      all(
        ...guides.map((guide) => all(guide.start(0, 0.7, easeInOutCubic), guide.end(1, 0.7, easeInOutCubic))),
        sequence(0.06, ...circles.map((circle) => circle.end(1, 0.6, easeInOutCubic))),
        delay(0.3, sequence(0.035, ...joints.map((joint) => joint.scale(1, 0.35, easeOutBack)))),
      ),
    ),
    delay(1, draw()),
  );

  function* draw() {
    yield* all(
      pens[0].scale(1, 0.15),
      v().end(1, 0.5, easeInOutCubic),
      delay(0.35, all(pens[1].scale(1, 0.15), un().end(1, 0.9, easeInOutCubic))),
      delay(0.45, pens[0].scale(0, 0.2)),
      delay(1.2, pens[1].scale(0, 0.2)),
    );
    yield* all(
      v().lineWidth(weight, 0.5, easeOutBack),
      delay(0.08, un().lineWidth(weight, 0.5, easeOutBack)),
      ...joints.map((joint) => joint.scale(0, 0.25, easeInCubic)),
      ...guides.map((guide) => guide.opacity(0, 0.3)),
      ...circles.map((circle) => circle.opacity(0, 0.3)),
      ...grid.map((line) => line.opacity(0, 0.6)),
      delay(0.35, land()),
      // While the echo still sways.
      ...(variables.get('mascot', false)() ? [delay(1.1, arrive())] : []),
    );
  }

  // The word squashes and the v wags, `force` as hard as when the word lands.
  function* land(force = 1) {
    const squash = (x: number, y: number): [number, number] => [1 + x * force, 1 + y * force];
    yield* all(
      word()
        .scale(squash(0.08, -0.12), 0.12, easeOutCubic)
        .to(squash(-0.04, 0.06), 0.16, easeInOutCubic)
        .to(squash(0.02, -0.02), 0.14, easeInOutCubic)
        .to([1, 1], 0.16, easeInOutCubic),
      // The echo, heavier and further back, answers late and slow.
      delay(
        0.3,
        sway()
          .rotation(-4, 0.35, easeOutCubic)
          .to(1.5, 0.45, easeInOutSine)
          .to(0, 0.5, easeInOutSine),
      ),
      delay(
        0.1,
        tip()
          // Mostly away from the u, which it would cross.
          .rotation(-9 * force, 0.14, easeOutCubic)
          .to(3 * force, 0.18, easeInOutSine)
          .to(-2 * force, 0.16, easeInOutSine)
          .to(0, 0.18, easeInOutSine),
      ),
    );
  }

  yield* waitFor(HOLD);

  // The word slides left to make room, the mascot drops into it, as tall as the letters and
  // standing on their bottom, a stroke's width from them; it lands, shaking the word, and cheers.
  function* arrive() {
    const body = extent();
    const size = (box.height - weight) / body.height;
    const width = body.width * size * SCALE;
    const gap = weight * SCALE;
    // The letters' right edge, from the middle of the view box.
    const edge = (box.width - weight / 2 - middle.x) * SCALE;
    const shift = (gap + width) / 2;
    const ground = lockup().y() + (weight / 2) * SCALE;
    const mascot = new Mascot(scheme, 'awe');
    mascot.root.scale(size * SCALE);
    mascot.root.position([edge + gap + width / 2 - shift, ground - DROP]);
    room().add(mascot.root);
    // The landing follows the fall at once, while the room still settles.
    yield* all(
      lockup().x(-shift, 0.6, easeInOutCubic),
      room().scale(ROOM, 0.9, easeInOutCubic),
      delay(
        0.3,
        chain(
          mascot.root.y(ground, 0.45, easeInCubic),
          all(mascot.land(0.2), land(0.4), delay(0.15, mascot.feel('glee', 0.35))),
        ),
      ),
    );
    yield* waitFor(0.3);
    yield* mascot.glance({ x: -10, y: 0 }, 0.35);
    yield* waitFor(0.4);
    yield* all(mascot.glance({ x: 0, y: 0 }, 0.3), delay(0.1, mascot.blink()));
  }
});
