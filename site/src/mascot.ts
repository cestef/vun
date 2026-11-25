// The mascot as `assets/motion/src/mascot/Mascot.tsx` draws and moves it, from the same shape
// and moods. Nested groups each hold one kind of motion, all turning on the middle of its ground
// line: `root` places and hops, `squash` squashes for a moment, `pose` holds the stretch and lean
// of a mood; the face is a value the features are drawn from, blended from mood to mood. `gaze`
// follows the pointer on top of where the face looks.

import {MOODS, type MoodName} from '../../assets/motion/src/mascot/moods';
import {type Face, MASCOT, type Point, eyePath, mouthPath, outline} from '../../assets/motion/src/mascot/shape';
import {type Move, Value, all, ease} from './motion';
import {Node} from './node';
import {make} from './svg';

// How far the eyes close in a blink, what is left of their height.
const SHUT = 0.1;

export class Mascot {
  readonly root: Node;
  readonly gaze: Value<Point>;
  private readonly squash: Node;
  private readonly pose: Node;
  private readonly face: Value<Face>;
  private readonly lids: Value<number>[];
  // Where the mascot stands on its ground: the height a hop returns to, or an interrupted one.
  ground = 0;
  // Whether a hop is in the air.
  hopping = false;

  private readonly eyes: SVGPathElement[];
  private readonly sockets: SVGGElement[];
  private readonly mouth: SVGPathElement;

  constructor(mood: MoodName) {
    const {view, weight, body, ground, face: at} = MASCOT;
    const feature = {'stroke-width': at.weight, 'stroke-linecap': 'round', 'stroke-linejoin': 'round'};
    this.eyes = [make('path', {...feature, fill: 'none'}), make('path', {...feature, fill: 'none'})];
    this.sockets = this.eyes.map((eye) => make('g', {}, eye));
    this.mouth = make('path', feature);
    const looking = make('g', {}, make('g', {class: 'face'}, ...this.sockets, this.mouth));

    this.lids = this.sockets.map(() => new Value(1, () => this.render()));
    this.face = new Value(MOODS[mood].face, () => this.render());
    this.gaze = new Value({x: 0, y: 0}, ({x, y}) => looking.setAttribute('transform', `translate(${x} ${y})`));
    this.gaze.set({x: 0, y: 0});

    const shape = make('path', {class: 'body', d: outline(body), 'stroke-width': weight, 'stroke-linejoin': 'round'});
    const drawing = make('g', {transform: `translate(${-view.width / 2} ${-ground})`}, shape, looking);
    this.pose = new Node([drawing]);
    this.squash = new Node([this.pose.group]);
    this.root = new Node([this.squash.group]);
    this.root.group.classList.add('mascot');
    this.hold(mood);
  }

  // Draws the face as it is now, each eye as open as its lid.
  private render() {
    const {face: at} = MASCOT;
    const {eyes, mouth, look} = this.face.get();
    for (const [i, side] of [-1, 1].entries()) {
      this.eyes[i].setAttribute('d', eyePath(eyes[i]));
      const x = at.eyes.x + side * at.eyes.spread + look.x;
      this.sockets[i].setAttribute('transform', `translate(${x} ${at.eyes.y + look.y}) scale(1 ${this.lids[i].get()})`);
    }
    this.mouth.setAttribute('d', mouthPath(mouth));
    this.mouth.setAttribute('transform', `translate(${at.mouth.x + look.x} ${at.mouth.y + look.y})`);
  }

  // Takes `mood` at once.
  hold(mood: MoodName) {
    const {face, pose} = MOODS[mood];
    this.face.set(face);
    this.pose.set({scale: [pose.stretch, 1 / pose.stretch], rotation: pose.lean});
  }

  // Where it stands now is its ground.
  stand() {
    this.ground = this.root.position.get().y;
  }

  // Ends a hop or a squash in progress, returning to the ground and opening the eyes.
  settle(duration = 0.12): Move {
    return all(
      this.root.to({y: this.ground}, duration, ease.outCubic),
      this.squash.to({scale: [1, 1]}, duration, ease.outCubic),
      ...this.lids.map((lid) => lid.to(1, duration, ease.outCubic)),
    );
  }

  // Blends into `mood`, blinking over the change of the eyes unless `blinking` is off.
  feel(mood: MoodName, duration = 0.4, blinking = true): Move {
    const {face, pose} = MOODS[mood];
    return all(
      this.face.to(face, duration, ease.inOutCubic),
      blinking ? this.blink(duration) : Promise.resolve(true),
      this.pose.scale.to([pose.stretch, 1 / pose.stretch], duration, ease.outBack),
      this.pose.rotation.to(pose.lean, duration, ease.inOutCubic),
    );
  }

  blink(duration = 0.3): Move {
    return all(
      ...this.lids.map((lid) =>
        lid.through([SHUT, duration * 0.35, ease.inCubic], [SHUT, duration * 0.15], [1, duration * 0.5, ease.outCubic]),
      ),
    );
  }

  // Looks towards `look`, in units of the view box.
  glance(look: Point, duration = 0.3): Move {
    return this.face.to({...this.face.get(), look}, duration, ease.inOutCubic);
  }

  // Squashes as if landing, `depth` of its height.
  land(depth = 0.14): Move {
    return this.squash.scale.through(
      [[1 + depth, 1 - depth], 0.1, ease.outCubic],
      [[1 - depth / 3, 1 + depth / 3], 0.14, ease.inOutCubic],
      [[1, 1], 0.16, ease.inOutCubic],
    );
  }

  // A hop of `height` units: crouches, rises stretched, falls and lands; false when cut short.
  async hop(height: number, air = 0.2): Move {
    const {ground} = this;
    this.hopping = true;
    try {
      return await this.leap(height, air, ground);
    } finally {
      this.hopping = false;
    }
  }

  private async leap(height: number, air: number, ground: number): Move {
    if (!(await this.squash.to({scale: [1.1, 0.9]}, 0.05, ease.outCubic))) return false;
    const up = await all(
      this.squash.scale.through([[0.93, 1.08], air * 0.6, ease.outCubic], [[1, 1], air * 0.4]),
      this.root.to({y: ground - height}, air, ease.outCubic),
    );
    if (!up) return false;
    const down = await all(this.root.to({y: ground}, air * 0.8, ease.inCubic), this.squash.to({scale: [0.95, 1.06]}, air * 0.8));
    return down && this.land();
  }

  // Swells and settles once, slowly, as in sleep.
  breathe(): Move {
    return this.squash.scale.through(
      [[1.04, 0.96], 0.8, ease.inOutSine],
      [[1, 1], 0.8, ease.inOutSine],
    );
  }
}
