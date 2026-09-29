// Reading the hand-drawn SVGs of `assets/logo`, so the scenes draw their geometry rather than a
// copy of it.

export class Svg {
  private readonly root: Element;

  constructor(source: string) {
    this.root = new DOMParser().parseFromString(source, 'image/svg+xml').documentElement;
  }

  // The size of its view box.
  get view(): {width: number; height: number} {
    const box = this.root.getAttribute('viewBox')?.split(' ').map(Number);
    if (box === undefined || box.length !== 4) {
      throw new Error('an SVG of the logo has a view box');
    }
    return {width: box[2], height: box[3]};
  }

  // The element with the id `id`.
  group(id: string): Element {
    const found = this.root.querySelector(`#${id}`);
    if (found === null) {
      throw new Error(`the SVG has no element ${id}`);
    }
    return found;
  }
}

// The data of each path under `element`, in order.
export function paths(element: Element): string[] {
  return [...element.querySelectorAll('path')].map((path) => path.getAttribute('d') ?? '');
}

// The circles under `element`.
export function circles(element: Element): {x: number; y: number; r: number}[] {
  return [...element.querySelectorAll('circle')].map((c) => ({
    x: Number(c.getAttribute('cx')),
    y: Number(c.getAttribute('cy')),
    r: Number(c.getAttribute('r')),
  }));
}

// The number an attribute of `element` holds.
export function number(element: Element, attribute: string): number {
  return Number(element.getAttribute(attribute));
}
