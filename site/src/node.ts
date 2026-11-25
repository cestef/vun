// A group placed, turned and scaled about a pivot of its own, as a node of the motion scenes: each
// of the three moves on its own, so a turn and a squash play together as they do there.

import {type Move, Value, all} from './motion';
import {make} from './svg';

export interface Place {
  x: number;
  y: number;
  // Degrees.
  rotation: number;
  scale: [number, number];
}

export const HOME: Place = {x: 0, y: 0, rotation: 0, scale: [1, 1]};

type Point = {x: number; y: number};

export class Node {
  readonly group: SVGGElement;
  readonly position: Value<Point>;
  readonly rotation: Value<number>;
  readonly scale: Value<[number, number]>;

  constructor(
    children: SVGElement[],
    at: Partial<Place> = {},
    private readonly pivot: Point = {x: 0, y: 0},
  ) {
    const {x, y, rotation, scale} = {...HOME, ...at};
    this.group = make('g', {}, ...children);
    const show = () => this.show();
    this.position = new Value({x, y}, show);
    this.rotation = new Value(rotation, show);
    this.scale = new Value(scale, show);
    this.show();
  }

  get(): Place {
    return {...this.position.get(), rotation: this.rotation.get(), scale: this.scale.get()};
  }

  // Moves the parts `change` names, each on its own, keeping the rest.
  to({rotation, scale, ...point}: Partial<Place>, seconds: number, easing?: (t: number) => number): Move {
    const moves: Move[] = [];
    if (point.x !== undefined || point.y !== undefined) moves.push(this.position.to({...this.position.get(), ...point}, seconds, easing));
    if (rotation !== undefined) moves.push(this.rotation.to(rotation, seconds, easing));
    if (scale !== undefined) moves.push(this.scale.to(scale, seconds, easing));
    return all(...moves);
  }

  set({rotation, scale, ...point}: Partial<Place>) {
    if (point.x !== undefined || point.y !== undefined) this.position.set({...this.position.get(), ...point});
    if (rotation !== undefined) this.rotation.set(rotation);
    if (scale !== undefined) this.scale.set(scale);
  }

  private show() {
    const {x, y, rotation, scale: [sx, sy]} = this.get();
    const {x: px, y: py} = this.pivot;
    this.group.setAttribute('transform', `translate(${x + px} ${y + py}) rotate(${rotation}) scale(${sx} ${sy}) translate(${-px} ${-py})`);
  }
}
