// A byte stream from the page to a worker that reads it blocking, as a WASI program reads its
// standard input: the page writes into shared memory what the worker has taken, the worker waits
// on it until something is there. It needs a page isolated across origins, where memory can be
// shared.

import {Fd} from '@bjorn3/browser_wasi_shim';

// How many bytes one write holds; longer messages are written in parts.
const CAPACITY = 1 << 20;
// The header: how many bytes are waiting, 0 once the worker took them.
const WAITING = 0;
const HEADER = Int32Array.BYTES_PER_ELEMENT;

export const shared = () => new SharedArrayBuffer(HEADER + CAPACITY);

// The page's end: bytes queued, written whenever the worker has taken the last.
export class Sender {
  private readonly header: Int32Array;
  private readonly data: Uint8Array;
  private queue: Uint8Array[] = [];

  constructor(memory: SharedArrayBuffer) {
    this.header = new Int32Array(memory, 0, 1);
    this.data = new Uint8Array(memory, HEADER);
  }

  send(bytes: Uint8Array) {
    this.queue.push(bytes);
    this.pump();
  }

  // Writes what fits of the queue once the worker has taken what was there; the worker says when.
  pump() {
    if (Atomics.load(this.header, WAITING) !== 0 || this.queue.length === 0) return;
    let written = 0;
    while (this.queue.length > 0 && written < CAPACITY) {
      const next = this.queue[0];
      const taken = Math.min(next.length, CAPACITY - written);
      this.data.set(next.subarray(0, taken), written);
      written += taken;
      if (taken === next.length) this.queue.shift();
      else this.queue[0] = next.subarray(taken);
    }
    Atomics.store(this.header, WAITING, written);
    Atomics.notify(this.header, WAITING);
  }
}

// The worker's end, as a WASI standard input: a read waits until bytes are there. `took` is
// called once the page may write again.
export class Receiver extends Fd {
  private readonly header: Int32Array;
  private readonly data: Uint8Array;
  private left = new Uint8Array();

  constructor(memory: SharedArrayBuffer, private readonly took: () => void) {
    super();
    this.header = new Int32Array(memory, 0, 1);
    this.data = new Uint8Array(memory, HEADER);
  }

  fd_read(size: number): {ret: number; data: Uint8Array} {
    if (this.left.length === 0) {
      while (Atomics.load(this.header, WAITING) === 0) Atomics.wait(this.header, WAITING, 0);
      this.left = this.data.slice(0, Atomics.load(this.header, WAITING));
      Atomics.store(this.header, WAITING, 0);
      this.took();
    }
    const data = this.left.slice(0, size);
    this.left = this.left.subarray(data.length);
    return {ret: 0, data};
  }
}
