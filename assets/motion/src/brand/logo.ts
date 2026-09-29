// The logo's geometry, from `wordmark/light.svg` and `construction.svg`.

import construction from '../../../logo/construction.svg?raw';
import wordmark from '../../../logo/wordmark/light.svg?raw';
import {Svg, circles, number, paths} from './svg';

const drawn = new Svg(wordmark);
const strokes = drawn.group('parts-wordmark').querySelector('g') as Element;
const [v, un] = paths(strokes);

const built = new Svg(construction);

export const LOGO = {
  view: drawn.view,
  weight: number(strokes, 'stroke-width'),
  v,
  un,
  // Where the letters stand, the line their strokes end on: the view box ends a stroke's width
  // below it, half for the round caps and half a margin.
  baseline: drawn.view.height - number(strokes, 'stroke-width'),
  construction: {
    grid: paths(built.group('grid')),
    guides: paths(built.group('guides')),
    circles: circles(built.group('circles')),
    // Where strokes end and turn, and the centers of the arcs.
    joints: circles(built.group('joints')),
    centers: circles(built.group('centers')),
    hairline: number(built.group('strokes'), 'stroke-width'),
  },
};
