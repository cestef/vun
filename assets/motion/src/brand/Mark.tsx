// The v alone, turning on its tip: small beside a name, or enlarged behind a card in the echo
// color of its scheme, swaying when the word in front lands.

import {Node, Path, Rect} from '@motion-canvas/2d';
import {type ThreadGenerator, Vector2, easeInOutSine, easeOutCubic} from '@motion-canvas/core';

import type {Frame} from '../clip/frame';
import {LOGO} from './logo';
import type {Scheme} from './palette';
import {turnOn} from './turn';

// Where a card places the echo: as tall as its shorter side, cut by its right edge at this share
// of the v's width and by its bottom edge at this share of its height.
const CUT = new Vector2(0.3, 0.12);

export class Mark {
  readonly root: Node;
  readonly v: Path;
  private readonly sway: Node;

  // Drawn `scale` pixels per unit of the view box, its origin at the root.
  constructor(color: string, scale: number) {
    // Drawn where it is placed, never laid out by a layout it stands in.
    this.v = (<Path layout={false} data={LOGO.v} stroke={color} lineWidth={LOGO.weight} lineCap="round" lineJoin="round" />) as Path;
    this.sway = (
      <Node>
        <Node>{this.v}</Node>
      </Node>
    ) as Node;
    this.root = (<Node scale={scale}>{this.sway}</Node>) as Node;
    turnOn(this.sway, this.v.getPointAtPercentage(0.5).position);
  }

  // The echo of `scheme`, placed as a card places it in `frame`.
  static echo(scheme: Scheme, frame: Frame): Mark {
    const echo = new Mark(scheme.echo, 1);
    const {position, size} = echo.extent;
    const scale = frame.side / size.y;
    echo.root.scale(scale);
    const box = size.scale(scale);
    const corner = frame.size.scale(0.5).add(box.mul(CUT)).sub(box);
    echo.root.position(corner.sub(position.scale(scale)));
    return echo;
  }

  // In `color`, `height` tall, centered in a box of its size for a layout to place.
  static boxed(color: string, height: number): {mark: Mark; box: Rect} {
    const mark = new Mark(color, 1);
    const {position, size} = mark.extent;
    const scale = height / size.y;
    mark.root.scale(scale);
    mark.root.position(position.add(size.scale(0.5)).scale(-scale));
    const box = (<Rect width={size.x * scale} height={height}>{mark.root}</Rect>) as Rect;
    return {mark, box};
  }

  // The v with its stroke, in units of the view box.
  get extent(): {position: Vector2; size: Vector2} {
    const {position, size} = this.v.cacheBBox();
    return {position, size};
  }

  // Sways on its tip, heavy and slow.
  *answer(): ThreadGenerator {
    yield* this.sway
      .rotation(-4, 0.35, easeOutCubic)
      .to(1.5, 0.45, easeInOutSine)
      .to(0, 0.5, easeInOutSine);
  }
}
