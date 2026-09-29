import {Layout, type Node} from '@motion-canvas/2d';

import type {Area} from './frame';

// The share of an area's width text wraps at.
const MEASURE = 0.85;

// `children` in a column against the left of `area`, in the middle of its height, `gap` apart.
export function column(area: Area, gap: number, children: Node[]): Layout {
  return (
    <Layout
      layout
      direction="column"
      gap={gap}
      width={area.size.x * MEASURE}
      offset={[-1, 0]}
      position={[area.start.x, area.center.y]}
    >
      {children}
    </Layout>
  ) as Layout;
}
