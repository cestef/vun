import type {Node} from '@motion-canvas/2d';
import type {Vector2} from '@motion-canvas/core';

// Makes `pivot` turn on `point`, in the space of its first child: the pivot moves there, the
// child back by as much.
export function turnOn(pivot: Node, point: Vector2) {
  pivot.position(point);
  pivot.children()[0].position(point.scale(-1));
}
