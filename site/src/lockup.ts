// The wordmark and the mascot as `assets/motion/src/brand/Wordmark.tsx` moves them in the short
// sting: the strokes traced by a pen each, the word lands and the v wags on its tip; the word
// slides aside to make room and the mascot drops in beside it. Units are those of the logo's view
// box, the word centered on the origin.

import {FALL, SHORT, TRACE} from '../../assets/motion/src/brand/beats';
import {MASCOT} from '../../assets/motion/src/mascot/shape';
import wordmark from '../../assets/logo/wordmark/light.svg?raw';
import {Mascot} from './mascot';
import {Latest, type Move, Value, after, all, ease, wait} from './motion';
import {Node} from './node';
import {make, read} from './svg';

const drawn = read(wordmark);
const strokes = drawn.querySelector('g[stroke-width]') as SVGGElement;
const [V, UN] = [...strokes.querySelectorAll('path')].map((path) => path.getAttribute('d') ?? '');
const [, , WIDTH, HEIGHT] = (drawn.getAttribute('viewBox') ?? '').split(' ').map(Number);
const WEIGHT = Number(strokes.getAttribute('stroke-width'));
// The line the letters stand on: a stroke's width above the view box's bottom.
const BASELINE = HEIGHT - WEIGHT;
const MIDDLE = {x: WIDTH / 2, y: HEIGHT / 2};

// The pen's dot; how far above its place the mascot drops from; the share of its size the word
// and the mascot together take, to fit.
const PEN = 7;
const DROP = 370;
const ROOM = 0.86;

// A stroke drawn up to `end` of its length, with a pen at its end. Browsers start a dash anew on
// each subpath, so each is a path of its own, drawn in turn.
class Stroke {
  readonly paths: SVGPathElement[];
  readonly pen: SVGCircleElement;
  readonly end: Value<number>;
  readonly lift: Value<number>;

  constructor(d: string, part: string) {
    this.paths = d.split(/(?=M)/).map((sub) => make('path', {d: sub.trim(), class: part, pathLength: 1}));
    this.pen = make('circle', {r: 0, class: `pen ${part}`});
    const lengths = this.paths.map((path) => path.getTotalLength());
    const total = lengths.reduce((sum, length) => sum + length, 0);
    this.end = new Value(1, (end) => {
      let left = end * total;
      for (const [i, path] of this.paths.entries()) {
        const drawn = Math.min(1, Math.max(0, left / lengths[i]));
        path.setAttribute('stroke-dasharray', `${drawn} 2`);
        path.style.visibility = drawn > 0 ? '' : 'hidden';
        if (left >= 0 && (left <= lengths[i] || i === this.paths.length - 1)) {
          const at = path.getPointAtLength(Math.min(left, lengths[i]));
          this.pen.setAttribute('cx', String(at.x));
          this.pen.setAttribute('cy', String(at.y));
        }
        left -= lengths[i];
      }
    });
    this.lift = new Value(0, (scale) => this.pen.setAttribute('r', String((PEN / 2) * scale)));
  }
}

export class Lockup {
  readonly svg: SVGSVGElement;
  readonly mascot: Mascot;
  // Covers the word and never moves with it, so that what the pointer is over does not change as the word squashes.
  readonly hit: SVGRectElement;
  private readonly landings = new Latest();
  private readonly stings = new Latest();
  // Whether the sting being played is still the one: ended by `skip`.
  private sting = () => true;
  private readonly root: Node;
  private readonly lockup: Node;
  private readonly word: Node;
  private readonly tip: Node;
  private readonly v: Stroke;
  private readonly un: Stroke;

  constructor() {
    this.v = new Stroke(V, 'v');
    this.un = new Stroke(UN, 'un');
    // The v turns on its middle, its tip.
    const [v] = this.v.paths;
    const {x, y} = v.getPointAtLength(v.getTotalLength() / 2);
    this.tip = new Node([...this.v.paths, this.v.pen], {}, {x, y});
    const canvas = make('g', {transform: `translate(${-MIDDLE.x} ${-MIDDLE.y})`, 'stroke-width': WEIGHT, fill: 'none', 'stroke-linecap': 'round', 'stroke-linejoin': 'round'}, this.tip.group, ...this.un.paths, this.un.pen);
    // The word squashes on the middle of its baseline.
    this.word = new Node([canvas], {}, {x: 0, y: BASELINE - MIDDLE.y});
    this.word.group.classList.add('word');
    this.hit = make('rect', {class: 'hit', x: -WIDTH / 2, y: -MIDDLE.y, width: WIDTH, height: HEIGHT, fill: 'transparent'});
    this.lockup = new Node([this.word.group, this.hit]);
    this.mascot = new Mascot('awe');
    this.root = new Node([this.lockup.group, this.mascot.root.group]);
    const room = this.room();
    const width = WIDTH * (1 + 0.5);
    this.svg = make('svg', {viewBox: `${-width / 2} ${-MIDDLE.y} ${width} ${HEIGHT}`, class: 'lockup', role: 'img', 'aria-label': 'vun'}, this.root.group);
    // It stands where the word leaves it room, in the root's space, which the word's slide leaves.
    this.mascot.root.set({x: room.x, y: BASELINE - MIDDLE.y + WEIGHT / 2, scale: [room.size, room.size]});
    this.mascot.stand();
    this.mascot.root.group.style.visibility = 'hidden';
  }

  // The mascot's size per unit of the view box, how far the word moves left to center the two,
  // and where the mascot stands then, before the word moves.
  private room(): {size: number; shift: number; x: number} {
    // The body with its outline, as `mascot/svg.ts` crops the stills to it; `svg.ts` itself reads
    // the scenes' palette, which needs Motion Canvas.
    const {body, weight} = MASCOT;
    const size = (HEIGHT - WEIGHT) / (body.base - body.top + weight);
    const width = (body.right - body.left + weight) * size;
    const edge = WIDTH / 2 - WEIGHT / 2;
    const shift = (WEIGHT + width) / 2;
    return {size, shift, x: edge + WEIGHT + width / 2 - shift};
  }

  // The word and the mascot where the sting leaves them, at once.
  settled() {
    this.v.end.set(1);
    this.un.end.set(1);
    this.v.lift.set(0);
    this.un.lift.set(0);
    this.word.set({scale: [1, 1]});
    this.tip.set({rotation: 0});
    this.mascot.root.set({y: this.mascot.ground});
    this.lockup.set({x: -this.room().shift});
    this.root.set({scale: [ROOM, ROOM]});
    this.mascot.root.group.style.visibility = '';
    this.mascot.hold('calm');
    this.mascot.stand();
  }

  // The short sting: traced at its pace, landed, then the mascot welcomed. Ended early by `skip`.
  async play(): Promise<void> {
    const live = (this.sting = this.stings.begin());
    this.v.end.set(0);
    this.un.end.set(0);
    if (!(await wait(SHORT.start)) || !live() || !(await this.draw(SHORT.pace))) return;
    await all(this.land(), after(SHORT.welcome, () => this.welcome(), live));
  }

  // Ends the sting where it is: everything stands as it does at its end.
  skip() {
    this.stings.begin();
    this.settled();
  }

  // Traces the v then the rest with a pen each, `pace` times as slow as the sting.
  draw(pace = 1): Move {
    const {pen, lift} = TRACE;
    return all(
      this.v.lift.to(1, pen * pace, ease.linear),
      this.v.end.to(1, TRACE.v * pace, ease.inOutCubic),
      after(
        TRACE.un.after * pace,
        () => all(this.un.lift.to(1, pen * pace, ease.linear), this.un.end.to(1, TRACE.un.length * pace, ease.inOutCubic)),
        this.sting,
      ),
      after(lift.v * pace, () => this.v.lift.to(0, lift.length * pace, ease.linear), this.sting),
      after(lift.un * pace, () => this.un.lift.to(0, lift.length * pace, ease.linear), this.sting),
    );
  }

  // The word squashes and the v wags, `force` as hard as when the word lands.
  land(force = 1): Move {
    const live = this.landings.begin();
    const squash = (x: number, y: number): [number, number] => [1 + x * force, 1 + y * force];
    const at = this.word.place.get();
    return all(
      this.word.place.through(
        [{...at, scale: squash(0.08, -0.12)}, 0.12, ease.outCubic],
        [{...at, scale: squash(-0.04, 0.06)}, 0.16, ease.inOutCubic],
        [{...at, scale: squash(0.02, -0.02)}, 0.14, ease.inOutCubic],
        [{...at, scale: [1, 1]}, 0.16, ease.inOutCubic],
      ),
      after(0.1, () =>
        this.tip.place.through(
          // Mostly away from the u, which it would cross.
          [{...this.tip.place.get(), rotation: -9 * force}, 0.14, ease.outCubic],
          [{...this.tip.place.get(), rotation: 3 * force}, 0.18, ease.inOutSine],
          [{...this.tip.place.get(), rotation: -2 * force}, 0.16, ease.inOutSine],
          [{...this.tip.place.get(), rotation: 0}, 0.18, ease.inOutSine],
        ),
        live,
      ),
    );
  }

  // The word slides left to make room, the mascot drops into it; it lands, shaking the word, and
  // cheers, looks at the word and back.
  async welcome(): Promise<void> {
    const {mascot, sting: live} = this;
    const ground = mascot.ground;
    mascot.root.set({y: ground - DROP});
    mascot.root.group.style.visibility = '';
    const fell = await all(
      this.lockup.to({x: -this.room().shift}, 0.6, ease.inOutCubic),
      this.root.to({scale: [ROOM, ROOM]}, 0.9, ease.inOutCubic),
      after(
        FALL.after,
        async () => {
          if (!(await mascot.root.to({y: ground}, FALL.length, ease.inCubic))) return false;
          return all(mascot.land(0.2), this.land(0.4), after(0.15, () => mascot.feel('glee', 0.35), live));
        },
        live,
      ),
    );
    if (!fell || !(await wait(0.3)) || !live() || !(await mascot.glance({x: -10, y: 0}, 0.35))) return;
    if (!(await wait(0.4)) || !live()) return;
    await all(mascot.glance({x: 0, y: 0}, 0.3), after(0.1, () => mascot.blink(), live));
  }
}
