// A few values moved over time, as the motion scenes move theirs: a tween is a value, a target,
// a duration and an easing, driven by one animation frame loop that runs only while any moves.
// Durations are in seconds, as in `assets/motion`.

import {mix} from '../../assets/motion/src/mascot/shape';

import {REDUCED} from './svg';

export type Ease = (t: number) => number;

// A move, resolved with whether it played through: false when another took its place.
export type Move = Promise<boolean>;

// The easings of Motion Canvas the scenes use.
export const ease = {
  linear: (t: number) => t,
  inCubic: (t: number) => t ** 3,
  outCubic: (t: number) => 1 - (1 - t) ** 3,
  inOutCubic: (t: number) => (t < 0.5 ? 4 * t ** 3 : 1 - (-2 * t + 2) ** 3 / 2),
  inOutSine: (t: number) => -(Math.cos(Math.PI * t) - 1) / 2,
  outBack: (t: number) => 1 + 2.70158 * (t - 1) ** 3 + 1.70158 * (t - 1) ** 2,
} satisfies Record<string, Ease>;

interface Tween {
  step(now: number): boolean;
}

const running = new Set<Tween>();
let frame = 0;

function tick(now: number) {
  for (const tween of running) {
    if (!tween.step(now)) running.delete(tween);
  }
  frame = running.size > 0 ? requestAnimationFrame(tick) : 0;
}

// A value that shows itself through `apply` each time it changes.
export class Value<T> {
  private moving: {cancel(): void} | undefined;

  // Shown first by `set`, or by the first frame of `to`.
  constructor(
    private current: T,
    private readonly apply: (value: T) => void,
  ) {}

  get(): T {
    return this.current;
  }

  set(value: T) {
    this.moving?.cancel();
    this.moving = undefined;
    this.show(value);
  }

  private show(value: T) {
    this.current = value;
    this.apply(value);
  }

  // Moves to `target` over `seconds`; stopped by the next `to` or `set`. Says whether it got there.
  to(target: T, seconds: number, easing: Ease = ease.inOutCubic): Move {
    this.moving?.cancel();
    if (REDUCED.matches || seconds <= 0) {
      this.moving = undefined;
      this.show(target);
      return Promise.resolve(true);
    }
    const from = this.current;
    return new Promise((done) => {
      let start = -1;
      let live = true;
      const tween: Tween = {
        step: (now) => {
          if (!live) return false;
          if (start < 0) start = now;
          const t = Math.min(1, (now - start) / (seconds * 1000));
          this.show(mix(from, target, easing(t)));
          if (t === 1) done(true);
          return t < 1;
        },
      };
      this.moving = {
        cancel: () => {
          live = false;
          done(false);
        },
      };
      running.add(tween);
      frame ||= requestAnimationFrame(tick);
    });
  }

  // Moves through each `[target, seconds, easing]` in turn, and stops where another move cut it.
  async through(...steps: [T, number, Ease?][]): Move {
    for (const [target, seconds, easing] of steps) if (!(await this.to(target, seconds, easing))) return false;
    return true;
  }
}

export const wait = (seconds: number): Move =>
  REDUCED.matches ? Promise.resolve(true) : new Promise((done) => setTimeout(() => done(true), seconds * 1000));

// Plays `then` after `seconds`, unless a newer move of its `Latest` came first.
export const after = async (seconds: number, then: () => Promise<unknown>, live: () => boolean = () => true): Move => {
  await wait(seconds);
  return live() && (await then()) !== false;
};

export const all = async (...moves: Promise<unknown>[]): Move => (await Promise.all(moves)).every((done) => done !== false);

// The newest of a kind of move: each `begin` ends the one before, which asks its token between steps.
export class Latest {
  private epoch = 0;

  begin(): () => boolean {
    const mine = ++this.epoch;
    return () => mine === this.epoch;
  }
}

// Resolves when the page is shown: a hidden tab runs no frames, so nothing is queued for it.
export const visible = () =>
  document.hidden
    ? new Promise<void>((done) => {
        const shown = () => {
          if (document.hidden) return;
          document.removeEventListener('visibilitychange', shown);
          done();
        };
        document.addEventListener('visibilitychange', shown);
      })
    : Promise.resolve();
