// A window as the code card draws it (`cards/code/card.typ`): a bar with three dots and a label
// over a body, on a rounded panel in the code colors. Its lengths are shares of the body's font
// size, as the card's are of its 10 points.

import {Circle, Rect, Txt} from '@motion-canvas/2d';
import {type ThreadGenerator, all, easeOutCubic} from '@motion-canvas/core';

import type {CodeScheme} from '../brand/code';
import {ADVANCE, FONT} from '../brand/type';
import type {Area} from './frame';

const CARD = {
  radius: 1,
  border: 0.075,
  dot: 0.64,
  bar: {x: 1.4, y: 0.9, gap: 0.5, label: 0.85, before: 1},
  body: {x: 1.8, top: 1.4, bottom: 1.8},
};
// The height of a line of the body, per unit of its font size.
export const LEADING = 1.5;
// The most a window's font takes of the frame's shorter side.
const LARGEST = 0.034;
// How far the window rises as it opens, per unit of the font size.
const RISE = 2;

export class Window {
  readonly root: Rect;
  // Where the content goes, sized for `columns` by `rows` characters.
  readonly body: Rect;

  constructor(
    colors: CodeScheme,
    label: string,
    private readonly font: number,
    columns: number,
    rows: number,
  ) {
    const dot = () => <Circle size={CARD.dot * font} fill={colors.border} />;
    this.body = (
      <Rect
        layout
        direction="column"
        width={(columns * ADVANCE + 2 * CARD.body.x) * font}
        height={(rows * LEADING + CARD.body.top + CARD.body.bottom) * font}
        padding={[CARD.body.top * font, CARD.body.x * font, CARD.body.bottom * font, CARD.body.x * font]}
      />
    ) as Rect;
    this.root = (
      <Rect
        layout
        direction="column"
        fill={colors.background}
        stroke={colors.border}
        lineWidth={CARD.border * font}
        radius={CARD.radius * font}
        clip
        opacity={0}
      >
        <Rect layout alignItems="center" gap={CARD.bar.gap * font} fill={colors.bar} padding={[CARD.bar.y * font, CARD.bar.x * font]}>
          {dot()}
          {dot()}
          {dot()}
          <Txt marginLeft={CARD.bar.before * font} text={label} fontFamily={FONT} fontSize={CARD.bar.label * font} fill={colors.comment} />
        </Rect>
        <Rect height={CARD.border * font} fill={colors.border} />
        {this.body}
      </Rect>
    ) as Rect;
  }

  // The font size of a window holding `columns` by `rows` characters that fits `area`, at most
  // its share of the frame's shorter `side`.
  static font(area: Area, side: number, columns: number, rows: number): number {
    const {bar, body} = CARD;
    const width = columns * ADVANCE + 2 * body.x;
    const height = rows * LEADING + body.top + body.bottom + 2 * bar.y + bar.label * LEADING + CARD.border;
    return Math.min(area.size.x / width, area.size.y / height, side * LARGEST);
  }

  // Rises into place as it appears.
  *open(duration = 0.5): ThreadGenerator {
    const at = this.root.y();
    this.root.y(at + RISE * this.font);
    yield* all(this.root.opacity(1, duration, easeOutCubic), this.root.y(at, duration, easeOutCubic));
  }
}
