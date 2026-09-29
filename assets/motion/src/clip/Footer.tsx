// Under a segment of a clip: its caption, for watching without sound, over a bar of the clip's
// segments, those played filled, this one filling as it plays.

import {Layout, Rect, Txt} from '@motion-canvas/2d';
import {clamp, useScene} from '@motion-canvas/core';

import type {Scheme} from '../brand/palette';
import {FONT} from '../brand/type';
import type {Placed} from './episode';
import type {Frame} from './frame';

// Shares of the shorter side: the caption's size, the bar's thickness, the gap between its
// pieces, and the room between caption and bar and above the caption.
const SIZE = {caption: 0.032, bar: 0.006, gap: 0.006, room: 0.025};
// The lines of caption kept room for, and their height per unit of its size.
const LINES = 2;
const LEADING = 1.3;

export class Footer {
  readonly root: Layout;

  // The height it takes at the bottom of the frame's area, with the room above it.
  static height(frame: Frame): number {
    return frame.side * (SIZE.caption * LEADING * LINES + 2 * SIZE.room + SIZE.bar);
  }

  constructor(scheme: Scheme, frame: Frame, {segment, index, count}: Placed) {
    const scene = useScene();
    const size = (share: number) => frame.side * share;
    // How far this scene has played, from its first frame to its last.
    const played = () =>
      clamp(0, 1, (scene.playback.frame - scene.firstFrame) / Math.max(1, scene.lastFrame - scene.firstFrame));
    const piece = (i: number) => (
      <Rect grow={1} height={size(SIZE.bar)} radius={size(SIZE.bar) / 2} fill={scheme.echo}>
        <Rect
          layout={false}
          offset={[-1, 0]}
          x={() => -((frame.area.size.x - (count - 1) * size(SIZE.gap)) / count) / 2}
          width={() => ((frame.area.size.x - (count - 1) * size(SIZE.gap)) / count) * (i < index ? 1 : i === index ? played() : 0)}
          height={size(SIZE.bar)}
          radius={size(SIZE.bar) / 2}
          fill={scheme.accent}
        />
      </Rect>
    );
    this.root = (
      <Layout
        layout
        direction="column"
        alignItems="center"
        gap={size(SIZE.room)}
        width={frame.area.size.x}
        offset={[0, 1]}
        y={frame.area.size.y / 2}
      >
        {segment.caption === undefined ? null : (
          <Txt fontFamily={FONT} fontSize={size(SIZE.caption)} fill={scheme.text} lineHeight={`${LEADING * 100}%`} textAlign="center" textWrap text={segment.caption} />
        )}
        <Layout gap={size(SIZE.gap)} width={frame.area.size.x}>
          {[...Array(count).keys()].map(piece)}
        </Layout>
      </Layout>
    ) as Layout;
  }
}
