// The mascot as SVG markup, for the files `scripts/mascot.mts` writes. Its parts carry the
// classes `body`, `face` and `eye`, which a style sheet paints and animates.

import type {Scheme} from '../brand/palette.ts';
import {type Face, MASCOT, eyePath, mouthPath, outline} from './shape.ts';

export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

// The body with its outline: what the stills show, cropped to it.
export function extent(): Box {
  const {body, weight} = MASCOT;
  const half = weight / 2;
  return {
    x: body.left - half,
    y: body.top - half,
    width: body.right - body.left + weight,
    height: body.base - body.top + weight,
  };
}

// The rules that paint the parts in `scheme`.
export function paint(scheme: Scheme): string {
  return `.body { fill: ${scheme.accent}; stroke: ${scheme.accent}; }
    .face { fill: ${scheme.background}; stroke: ${scheme.background}; }`;
}

// The body and a face: `faces` more than one stacks them, each in a group of class `mood`.
export function mascot(...faces: Face[]): string {
  const {weight, body, face: at} = MASCOT;
  const features = ({eyes, mouth, look}: Face) => {
    const place = (x: number, y: number) => `translate(${x + look.x},${y + look.y})`;
    const eye = (i: 0 | 1, side: number) =>
      `<g transform="${place(at.eyes.x + side * at.eyes.spread, at.eyes.y)}"><path class="eye" d="${eyePath(eyes[i])}" fill="none" /></g>`;
    return `${eye(0, -1)}${eye(1, 1)}<path d="${mouthPath(mouth)}" transform="${place(at.mouth.x, at.mouth.y)}" />`;
  };
  const groups = faces.length === 1 ? features(faces[0]) : faces.map((f) => `<g class="mood">${features(f)}</g>`).join('');
  return `<g class="mascot" stroke-linecap="round" stroke-linejoin="round">
    <path class="body" d="${outline(body)}" stroke-width="${weight}" />
    <g class="face" stroke-width="${at.weight}">${groups}</g>
  </g>`;
}

// A document of `box` holding `content`, styled by `style`.
export function document(box: Box, style: string, content: string): string {
  const {x, y, width, height} = box;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${x} ${y} ${width} ${height}" width="${width}" height="${height}">
  <style>
    ${style}
  </style>
  ${content}
</svg>
`;
}
