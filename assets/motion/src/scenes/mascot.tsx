// The mascot and its moods: it drops in, looks around, goes through each mood with the motion
// that carries it, falls asleep and wakes, then its moods line up beside it.

import {makeScene2D} from '@motion-canvas/2d';
import {
  all,
  delay,
  easeInCubic,
  easeInOutCubic,
  easeOutBack,
  easeOutCubic,
  sequence,
  useRandom,
  useScene,
  waitFor,
} from '@motion-canvas/core';

import {scheme as named} from '../brand/palette';
import {Mascot} from '../mascot/Mascot';
import {MOODS, type MoodName} from '../mascot/moods';
import {MASCOT} from '../mascot/shape';

// Pixels per unit of the view box, alone and in the line.
const SCALE = 3.4;
const LINED = 1.35;
// The distance between the mascots of the line.
const PITCH = 250;
// How far above the frame the mascot drops from.
const DROP = 900;

// Where the ground sits for a mascot of `scale` whose body is centered in the frame.
const ground = (scale: number) => (MASCOT.ground - MASCOT.middle) * scale;

export default makeScene2D(function* (view) {
  const scheme = named(useScene().variables.get('scheme', 'dark')());
  view.fill(scheme.background);
  const random = useRandom();

  const moods = Object.keys(MOODS) as MoodName[];
  // The line, the mascot of the first mood in its middle.
  const middle = Math.floor(moods.length / 2);
  const order = [...moods.slice(1, middle + 1), moods[0], ...moods.slice(middle + 1)];
  const line = order.map(() => new Mascot(scheme, 'calm'));
  const hero = line[middle];
  for (const mascot of line) {
    if (mascot !== hero) {
      mascot.root.scale(0);
      mascot.root.position([0, ground(LINED)]);
      view.add(mascot.root);
    }
  }
  view.add(hero.root);

  // Drops in stretched, lands, wakes to the frame.
  hero.root.scale(SCALE);
  hero.root.position([0, ground(SCALE) - DROP]);
  hero.hold('sleep');
  yield* hero.root.y(ground(SCALE), 0.5, easeInCubic);
  yield* all(hero.land(0.2), delay(0.2, hero.feel('calm', 0.35)));

  yield* waitFor(0.3);
  yield* hero.glance({x: -8, y: 0});
  yield* waitFor(0.35);
  yield* hero.glance({x: 8, y: -2}, 0.4);
  yield* waitFor(0.3);
  yield* all(hero.glance({x: 0, y: 0}), delay(0.15, hero.blink()));
  yield* waitFor(0.3);

  yield* all(hero.feel('glee'), delay(0.1, hero.hop(120)));
  yield* hero.hop(70, 0.2);
  yield* waitFor(0.3);

  yield* hero.feel('wink', 0.35);
  yield* waitFor(0.6);

  yield* all(hero.feel('awe', 0.25), hero.land(-0.1));
  yield* waitFor(0.6);

  yield* hero.feel('sad', 0.8);
  yield* waitFor(0.7);

  yield* all(hero.feel('cross', 0.3), delay(0.15, hero.shake(3, 10)));
  yield* waitFor(0.5);

  yield* hero.feel('sleep', 0.9);
  yield* hero.breathe(1);

  // Startled awake, then calm again.
  yield* all(hero.feel('awe', 0.15), hero.hop(90, 0.22));
  yield* hero.feel('calm', 0.35);
  yield* waitFor(0.3);

  // Shrinks into the middle of the line; the others slide out from behind it, then each takes
  // its mood in a wave from the left.
  yield* all(hero.root.scale(LINED, 0.6, easeInOutCubic), hero.root.y(ground(LINED), 0.6, easeInOutCubic));
  const outwards = [...line.keys()].filter((i) => i !== middle).sort((a, b) => Math.abs(a - middle) - Math.abs(b - middle));
  yield* sequence(
    0.07,
    ...outwards.map((i) =>
      all(line[i].root.scale(LINED, 0.45, easeOutBack), line[i].root.x((i - middle) * PITCH, 0.6, easeOutCubic)),
    ),
  );
  yield* sequence(0.12, ...line.map((mascot, i) => all(mascot.feel(order[i]), mascot.hop(40, 0.18))));
  yield* all(...line.map((mascot) => delay(random.nextFloat(0.3, 1.2), mascot.blink())));
  yield* waitFor(0.8);
});
