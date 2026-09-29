// The frame a clip's scene lays out in, and areas of it. Every length is a share of the frame's
// shorter side, as on the cards, so a scene agrees with itself at every size of the project.

import {Vector2} from '@motion-canvas/core';

// The share of the shorter side kept clear at each edge.
const MARGIN = 0.08;

// A region of the frame: its size, and its middle from the frame's.
export class Area {
  constructor(
    readonly size: Vector2,
    readonly center: Vector2,
  ) {}

  // Its top left corner, from the frame's middle.
  get start(): Vector2 {
    return this.center.sub(this.size.scale(0.5));
  }

  // Pixels per unit of `box` for it to take `share` of the width or of the height, whichever it
  // reaches first.
  fit(box: {width: number; height: number}, share: number): number {
    return Math.min((this.size.x * share) / box.width, (this.size.y * share) / box.height);
  }

  // Less `height` at the bottom.
  above(height: number): Area {
    return new Area(this.size.addY(-height), this.center.addY(-height / 2));
  }

  // Two parts, side by side when `across`, one above the other otherwise, `gap` between them.
  halves(gap: number, across: boolean): [Area, Area] {
    const wide = across;
    const along = wide ? Vector2.right : Vector2.down;
    const half = this.size.sub(along.scale(gap)).mul(wide ? new Vector2(0.5, 1) : new Vector2(1, 0.5));
    const offset = along.scale((gap + (wide ? half.x : half.y)) / 2);
    return [new Area(half, this.center.sub(offset)), new Area(half, this.center.add(offset))];
  }
}

export class Frame {
  constructor(readonly size: Vector2) {}

  get side(): number {
    return Math.min(this.size.x, this.size.y);
  }

  get margin(): number {
    return this.side * MARGIN;
  }

  // Within the margins.
  get area(): Area {
    return new Area(this.size.sub(new Vector2(2 * this.margin)), Vector2.zero);
  }
}
