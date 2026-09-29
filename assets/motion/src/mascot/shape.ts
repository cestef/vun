// The mascot's geometry, in the units of the logo's view box. Its body is a box whose corners
// are arcs, filled and outlined at `weight`, so the outline rounds it as a stroke rounds the
// letters; its face is drawn at a lighter weight, round-capped like them.

export interface Point {
  x: number;
  y: number;
}

// An eye, around its center: a stroke from `from` to `to` whose middle passes through `bend`.
export interface Eye {
  from: Point;
  bend: Point;
  to: Point;
}

// The mouth, around its center: two curves between its corners, `width` apart, whose middles
// sit `top` and `bottom` below the corners; the space between them is filled.
export interface Mouth {
  width: number;
  top: number;
  bottom: number;
}

// Where the face looks: an offset of all its features.
export interface Face {
  eyes: [Eye, Eye];
  mouth: Mouth;
  look: Point;
}

// The body's outline, and the radii of its corners from the top left, clockwise.
export interface Box {
  left: number;
  right: number;
  top: number;
  base: number;
  corners: [number, number, number, number];
}

// What `tools/body.html` designs; it prints this constant back to paste over it.
const DESIGN = {
  weight: 60,
  body: {left: 50, right: 150, top: 80, base: 160, corners: [0, 0, 0, 0]} as Box,
  face: {
    weight: 10,
    // The eyes' centers sit `spread` either side of the middle.
    eyes: {x: 100, y: 90, spread: 40},
    mouth: {x: 100, y: 110},
  },
};

export const MASCOT = {
  view: {width: 200, height: 200},
  ...DESIGN,
  // The line it stands on, where the outline of its base ends, and the height of its middle.
  ground: DESIGN.body.base + DESIGN.weight / 2,
  middle: (DESIGN.body.top + DESIGN.body.base) / 2,
};

// The right eye as the mirror of the left.
export function mirror(eye: Eye): Eye {
  const flip = ({x, y}: Point): Point => ({x: -x, y});
  return {from: flip(eye.from), bend: flip(eye.bend), to: flip(eye.to)};
}

// The radii of the corners of `box`, shrunk together where they would overlap, as CSS does.
export function radii({left, right, top, base, corners}: Box): Box['corners'] {
  const [tl, tr, br, bl] = corners;
  const w = right - left;
  const h = base - top;
  const fit = Math.min(1, w / (tl + tr || 1), w / (bl + br || 1), h / (tl + bl || 1), h / (tr + br || 1));
  return corners.map((r) => r * fit) as Box['corners'];
}

export function outline(box: Box): string {
  const {left, right, top, base} = box;
  const [tl, tr, br, bl] = radii(box);
  const arc = (r: number, x: number, y: number) => (r ? ` A${r},${r} 0 0 1 ${x},${y}` : '');
  return (
    `M${left + tl},${top} H${right - tr}${arc(tr, right, top + tr)} V${base - br}${arc(br, right - br, base)}` +
    ` H${left + bl}${arc(bl, left, base - bl)} V${top + tl}${arc(tl, left + tl, top)} Z`
  );
}

// The quadratic control point that makes a curve from `a` to `b` pass through `middle`.
function control(a: Point, middle: Point, b: Point): Point {
  return {x: 2 * middle.x - (a.x + b.x) / 2, y: 2 * middle.y - (a.y + b.y) / 2};
}

export function eyePath({from, bend, to}: Eye): string {
  const c = control(from, bend, to);
  return `M${from.x},${from.y} Q${c.x},${c.y} ${to.x},${to.y}`;
}

export function mouthPath({width, top, bottom}: Mouth): string {
  const w = width / 2;
  return `M${-w},0 Q0,${2 * top} ${w},0 Q0,${2 * bottom} ${-w},0 Z`;
}

// Every number of `a` moved towards `b` by `t`: faces, or any shape of numbers, blend.
export function mix<T>(a: T, b: T, t: number): T {
  if (typeof a === 'number') {
    return (a + ((b as number) - a) * t) as T;
  }
  if (Array.isArray(a)) {
    return a.map((item, i) => mix(item, (b as unknown[])[i], t)) as T;
  }
  const out = {} as Record<string, unknown>;
  for (const key in a) {
    out[key] = mix(a[key], (b as Record<string, unknown>)[key], t);
  }
  return out as T;
}
