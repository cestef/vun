// A value that views read and controllers write: a signal with the shape of a property. A controller
// is made inside a component and writes as it starts, which is why a cell is owned-write.

import {createSignal} from 'solid-js';

export class Cell<T> {
  private readonly read: () => T;
  private readonly write: (value: () => T) => void;

  constructor(initial: T) {
    // A function is a value here, as the setter's updater form would make it a computation.
    const [read, write] = createSignal(initial as never, {ownedWrite: true});
    this.read = read as () => T;
    this.write = write as (value: () => T) => void;
  }

  get value(): T {
    return this.read();
  }

  set value(value: T) {
    this.write(() => value);
  }
}
