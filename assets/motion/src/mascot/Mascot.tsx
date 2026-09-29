// The mascot drawn and moved. Nested nodes each hold one kind of motion, all turning on the
// middle of its ground line: `root` places and hops, `squash` squashes for a moment, `pose`
// holds the stretch and lean of a mood; the face is a signal the features draw from, blended
// from mood to mood.

import {Node, Path} from '@motion-canvas/2d';
import {
  type SimpleSignal,
  type ThreadGenerator,
  Vector2,
  all,
  createSignal,
  easeInCubic,
  easeInOutCubic,
  easeInOutSine,
  easeOutBack,
  easeOutCubic,
  loop,
} from '@motion-canvas/core';

import type {Scheme} from '../brand/palette';
import {MOODS, type MoodName} from './moods';
import {type Face, type Point, MASCOT, eyePath, mix, mouthPath, outline} from './shape';

// How far the eyes close in a blink, what is left of their height.
const SHUT = 0.1;

export class Mascot {
  readonly root: Node;
  readonly face: SimpleSignal<Face>;
  private readonly squash: Node;
  private readonly pose: Node;
  private readonly eyes: Node[];

  constructor(scheme: Scheme, mood: MoodName) {
    const {view, weight, body, ground, face: at} = MASCOT;
    this.face = createSignal(MOODS[mood].face);
    const feature = {
      stroke: scheme.background,
      lineWidth: at.weight,
      lineCap: 'round',
      lineJoin: 'round',
    } as const;
    const eye = (i: 0 | 1, side: number) => (
      <Node x={at.eyes.x + side * at.eyes.spread} y={at.eyes.y}>
        <Path data={() => eyePath(this.face().eyes[i])} {...feature} />
      </Node>
    );
    this.eyes = [eye(0, -1), eye(1, 1)];
    this.pose = (
      <Node>
        <Node x={-view.width / 2} y={-ground}>
          <Path data={outline(body)} fill={scheme.accent} stroke={scheme.accent} lineWidth={weight} lineJoin="round" />
          <Node position={() => new Vector2(this.face().look.x, this.face().look.y)}>
            {this.eyes}
            <Path x={at.mouth.x} y={at.mouth.y} data={() => mouthPath(this.face().mouth)} fill={scheme.background} {...feature} />
          </Node>
        </Node>
      </Node>
    ) as Node;
    this.squash = (<Node>{this.pose}</Node>) as Node;
    this.root = (<Node>{this.squash}</Node>) as Node;
    this.hold(mood);
  }

  // Takes `mood` at once.
  hold(mood: MoodName) {
    const {face, pose} = MOODS[mood];
    this.face(face);
    this.pose.scale([pose.stretch, 1 / pose.stretch]);
    this.pose.rotation(pose.lean);
  }

  // Blends into `mood`, blinking over the change of the eyes.
  *feel(mood: MoodName, duration = 0.4): ThreadGenerator {
    const {face, pose} = MOODS[mood];
    yield* all(
      this.face(face, duration, easeInOutCubic, mix),
      this.blink(duration),
      this.pose.scale([pose.stretch, 1 / pose.stretch], duration, easeOutBack),
      this.pose.rotation(pose.lean, duration, easeInOutCubic),
    );
  }

  *blink(duration = 0.3): ThreadGenerator {
    yield* all(
      ...this.eyes.map((eye) =>
        eye.scale.y(SHUT, duration * 0.35, easeInCubic).wait(duration * 0.15).to(1, duration * 0.5, easeOutCubic),
      ),
    );
  }

  // Looks towards `look`, in units of the view box.
  *glance(look: Point, duration = 0.3): ThreadGenerator {
    yield* this.face({...this.face(), look}, duration, easeInOutCubic, mix);
  }

  // Squashes as if landing, `depth` of its height.
  *land(depth = 0.14): ThreadGenerator {
    yield* this.squash
      .scale([1 + depth, 1 - depth], 0.1, easeOutCubic)
      .to([1 - depth / 3, 1 + depth / 3], 0.14, easeInOutCubic)
      .to([1, 1], 0.16, easeInOutCubic);
  }

  // A hop of `height` pixels: crouches, rises stretched, falls and lands.
  *hop(height: number, air = 0.26): ThreadGenerator {
    const y = this.root.y();
    yield* this.squash.scale([1.1, 0.9], 0.08, easeOutCubic);
    yield* all(
      this.squash.scale([0.93, 1.08], air * 0.6, easeOutCubic).to([1, 1], air * 0.4),
      this.root.y(y - height, air, easeOutCubic),
    );
    yield* all(this.root.y(y, air * 0.8, easeInCubic), this.squash.scale([0.95, 1.06], air * 0.8));
    yield* this.land();
  }

  // Shakes on the spot `times`, `reach` pixels either side.
  *shake(times: number, reach: number): ThreadGenerator {
    const x = this.root.x();
    yield* loop(times, () => this.root.x(x - reach, 0.04).to(x + reach, 0.08).to(x, 0.04));
  }

  // Swells and settles `times`, slowly, as in sleep.
  *breathe(times: number): ThreadGenerator {
    yield* loop(times, () =>
      this.squash.scale([1.04, 0.96], 0.8, easeInOutSine).to([1, 1], 0.8, easeInOutSine),
    );
  }
}
