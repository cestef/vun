// A group placed, turned and scaled about a pivot of its own, as a node of the motion scenes.

import {type Move, Value} from './motion';
import {make} from './svg';

export interface Place {
  x: number;
  y: number;
  // Degrees.
  rotation: number;
  scale: [number, number];
}

export const HOME: Place = {x: 0, y: 0, rotation: 0, scale: [1, 1]};

export class Node {
  readonly group: SVGGElement;
  readonly place: Value<Place>;

  constructor(
    children: SVGElement[],
    at: Partial<Place> = {},
    private readonly pivot = {x: 0, y: 0},
  ) {
    this.group = make('g', {}, ...children);
    this.place = new Value({...HOME, ...at}, ({x, y, rotation, scale: [sx, sy]}) => {
      const {x: px, y: py} = this.pivot;
      this.group.setAttribute(
        'transform',
        `translate(${x + px} ${y + py}) rotate(${rotation}) scale(${sx} ${sy}) translate(${-px} ${-py})`,
      );
    });
    this.place.set(this.place.get());
  }

  // Moves the part `change` names, keeping the rest.
  to(change: Partial<Place>, seconds: number, easing?: (t: number) => number): Move {
    return this.place.to({...this.place.get(), ...change}, seconds, easing);
  }

  set(change: Partial<Place>) {
    this.place.set({...this.place.get(), ...change});
  }
}
