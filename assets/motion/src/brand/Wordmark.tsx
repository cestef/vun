// The wordmark drawn and moved. `canvas` holds the strokes in units of the view box, from its top
// left; the word stands centered on the root, on its baseline. Nested nodes each hold one kind of
// motion: `root` makes room for the mascot, `lockup` slides aside for it, `word` squashes as it
// lands, `tip` wags the v on its tip.

import {Circle, Node, Path} from '@motion-canvas/2d';
import {
  type ThreadGenerator,
  Vector2,
  all,
  chain,
  delay,
  easeInCubic,
  easeInOutCubic,
  easeInOutSine,
  easeOutBack,
  easeOutCubic,
  waitFor,
} from '@motion-canvas/core';

import {Mascot} from '../mascot/Mascot';
import type {MoodName} from '../mascot/moods';
import {extent} from '../mascot/svg';
import type {Mark} from './Mark';
import {FALL, TRACE} from './beats';
import {LOGO} from './logo';
import type {Scheme} from './palette';
import {turnOn} from './turn';

// The pen's dot, in units of the view box.
const PEN = 7;
// The mascot drops from this far above its place, in units of the view box, and the word and the
// mascot together take this much of the word's size alone, to fit.
const DROP = 370;
const ROOM = 0.86;

export interface Options {
  // The width of the strokes, in units of the view box; the logo's weight unless drawn thinner
  // to swell later.
  lineWidth?: number;
  // The echo behind, which answers when the word lands.
  echo?: Mark;
}

export class Wordmark {
  readonly root: Node;
  readonly canvas: Node;
  readonly v: Path;
  readonly un: Path;
  private readonly pens: [Circle, Circle];
  private readonly lockup: Node;
  private readonly word: Node;
  private readonly tip: Node;
  private readonly echo: Mark | undefined;

  // Drawn `scale` pixels per unit of the view box.
  constructor(
    private readonly scheme: Scheme,
    private readonly scale: number,
    {lineWidth = LOGO.weight, echo}: Options = {},
  ) {
    const {view: box, baseline} = LOGO;
    const middle = new Vector2(box.width / 2, box.height / 2);
    const stroke = {lineWidth, lineCap: 'round', lineJoin: 'round'} as const;
    this.echo = echo;
    this.v = (<Path data={LOGO.v} stroke={scheme.accent} {...stroke} />) as Path;
    this.un = (<Path data={LOGO.un} stroke={scheme.ink} {...stroke} />) as Path;
    this.pens = [
      (<Circle size={PEN} fill={scheme.accent} scale={0} />) as Circle,
      (<Circle size={PEN} fill={scheme.ink} scale={0} />) as Circle,
    ];
    this.tip = (
      <Node>
        <Node>
          {this.v}
          {this.pens[0]}
        </Node>
      </Node>
    ) as Node;
    this.canvas = (
      <Node scale={scale} x={-middle.x * scale} y={-baseline * scale}>
        {this.tip}
        {this.un}
        {this.pens[1]}
      </Node>
    ) as Node;
    this.word = (<Node>{this.canvas}</Node>) as Node;
    this.lockup = (<Node y={(baseline - middle.y) * scale}>{this.word}</Node>) as Node;
    this.root = (<Node>{this.lockup}</Node>) as Node;

    turnOn(this.tip, this.v.getPointAtPercentage(0.5).position);
    for (const [pen, path] of [
      [this.pens[0], this.v],
      [this.pens[1], this.un],
    ] as const) {
      pen.position(() => path.getPointAtPercentage(path.end()).position);
    }
  }

  // Hides the strokes, for `draw` to trace them.
  erase() {
    this.v.end(0);
    this.un.end(0);
  }

  // Traces the v then the rest with a pen each, `pace` times as slow as the sting.
  *draw(pace = 1): ThreadGenerator {
    const [v, un] = this.pens;
    const {pen, lift} = TRACE;
    yield* all(
      v.scale(1, pen * pace),
      this.v.end(1, TRACE.v * pace, easeInOutCubic),
      delay(TRACE.un.after * pace, all(un.scale(1, pen * pace), this.un.end(1, TRACE.un.length * pace, easeInOutCubic))),
      delay(lift.v * pace, v.scale(0, lift.length * pace)),
      delay(lift.un * pace, un.scale(0, lift.length * pace)),
    );
  }

  // Swells the strokes to the logo's weight.
  *swell(): ThreadGenerator {
    yield* all(
      this.v.lineWidth(LOGO.weight, 0.5, easeOutBack),
      delay(0.08, this.un.lineWidth(LOGO.weight, 0.5, easeOutBack)),
    );
  }

  // The word squashes and the v wags, `force` as hard as when the word lands; the echo answers.
  *land(force = 1): ThreadGenerator {
    const squash = (x: number, y: number): [number, number] => [1 + x * force, 1 + y * force];
    yield* all(
      this.word
        .scale(squash(0.08, -0.12), 0.12, easeOutCubic)
        .to(squash(-0.04, 0.06), 0.16, easeInOutCubic)
        .to(squash(0.02, -0.02), 0.14, easeInOutCubic)
        .to([1, 1], 0.16, easeInOutCubic),
      ...(this.echo === undefined ? [] : [delay(0.3, this.echo.answer())]),
      delay(
        0.1,
        this.tip
          // Mostly away from the u, which it would cross.
          .rotation(-9 * force, 0.14, easeOutCubic)
          .to(3 * force, 0.18, easeInOutSine)
          .to(-2 * force, 0.16, easeInOutSine)
          .to(0, 0.18, easeInOutSine),
      ),
    );
  }

  // The mascot beside the word, as tall as the letters and standing on their bottom, a stroke's
  // width from them, in `mood`; placed where it stands once the word has made room.
  beside(mood: MoodName): Mascot {
    const {size, x} = this.room();
    const mascot = new Mascot(this.scheme, mood);
    mascot.root.scale(size * this.scale);
    mascot.root.position([x, this.lockup.y() + (LOGO.weight / 2) * this.scale]);
    this.root.add(mascot.root);
    return mascot;
  }

  // Makes the room `beside` stands in at once.
  roomy() {
    this.lockup.x(-this.room().shift);
    this.root.scale(ROOM);
  }

  // The word slides left to make room, the mascot drops into it; it lands, shaking the word, and
  // cheers, looks at the word and back.
  *welcome(): ThreadGenerator {
    const mascot = this.beside('awe');
    const ground = mascot.root.y();
    mascot.root.y(ground - DROP * this.scale);
    // The landing follows the fall at once, while the room still settles.
    yield* all(
      this.lockup.x(-this.room().shift, 0.6, easeInOutCubic),
      this.root.scale(ROOM, 0.9, easeInOutCubic),
      delay(
        FALL.after,
        chain(
          mascot.root.y(ground, FALL.length, easeInCubic),
          all(mascot.land(0.2), this.land(0.4), delay(0.15, mascot.feel('glee', 0.35))),
        ),
      ),
    );
    yield* waitFor(0.3);
    yield* mascot.glance({x: -10, y: 0}, 0.35);
    yield* waitFor(0.4);
    yield* all(mascot.glance({x: 0, y: 0}, 0.3), delay(0.1, mascot.blink()));
  }

  // The mascot's size per unit of the view box, how far the word moves left to center the two,
  // and where the mascot stands then, in the root's space.
  private room(): {size: number; shift: number; x: number} {
    const {view: box, weight} = LOGO;
    const body = extent();
    const size = (box.height - weight) / body.height;
    const width = body.width * size * this.scale;
    const gap = weight * this.scale;
    // The letters' right edge, from the middle of the view box.
    const edge = (box.width / 2 - weight / 2) * this.scale;
    const shift = (gap + width) / 2;
    return {size, shift, x: edge + gap + width / 2 - shift};
  }
}
