// Making SVG elements, and whether the reader asked for less motion.

const NS = 'http://www.w3.org/2000/svg';

export function read(text: string): SVGSVGElement {
  return new DOMParser().parseFromString(text, 'image/svg+xml').documentElement as unknown as SVGSVGElement;
}

export function make<K extends keyof SVGElementTagNameMap>(
  tag: K,
  attributes: Record<string, string | number> = {},
  ...children: SVGElement[]
): SVGElementTagNameMap[K] {
  const element = document.createElementNS(NS, tag);
  for (const [name, value] of Object.entries(attributes)) element.setAttribute(name, String(value));
  element.append(...children);
  return element;
}

export const REDUCED = matchMedia('(prefers-reduced-motion: reduce)');
