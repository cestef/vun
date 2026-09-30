// What every page has: the mark in the bar, the theme button.

import source from '../../assets/logo/mark/adaptive.svg?raw';
import {make, read} from './svg';
import {themed} from './theme';

// The mark's stroke without its own style, which would reach every path of the page.
function mark(): SVGSVGElement {
  const drawn = read(source);
  const svg = make('svg', {viewBox: drawn.getAttribute('viewBox') ?? '', class: 'mark', 'aria-hidden': 'true'});
  for (const path of drawn.querySelectorAll('path')) {
    const copy = path.cloneNode() as SVGPathElement;
    copy.setAttribute('class', 'v');
    svg.append(copy);
  }
  return svg;
}

export function chrome() {
  for (const slot of document.querySelectorAll('[data-mark]')) slot.replaceWith(mark());
  themed(document.querySelector<HTMLButtonElement>('button.theme'));
}
