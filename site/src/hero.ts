// The hero's lockup and what it does when played with: the mascot follows the pointer with its
// eyes, cheers when pointed at, hops into another mood when clicked, and falls asleep when left
// alone; pointing at the word makes it land again.

import type {MoodName} from '../../assets/motion/src/mascot/moods';
import {Lockup} from './lockup';
import {after, wait} from './motion';
import {REDUCED} from './svg';

// The moods a click shows, in turn.
const CLICKED: MoodName[] = ['wink', 'awe', 'cross', 'sad', 'glee'];
// How far the eyes follow the pointer, in units of the view box, and how far from the mascot it
// is followed at most, in pixels.
const GAZE = {x: 8, y: 5, reach: 500};
// How long a clicked mood holds, and how long without the pointer until it sleeps, in seconds.
const HOLD = 1.4;
const DROWSY = 25;
// How long between blinks when idle, in seconds: at least `least`, up to `spread` more.
const BLINK = {least: 3, spread: 4};
// How high a click makes it hop, in units of the view box.
const HOP = 45;

export async function hero(slot: HTMLElement) {
  const lockup = new Lockup();
  slot.append(lockup.svg);
  const {mascot} = lockup;
  const body = mascot.root.group;
  const word = lockup.svg.querySelector('.word') as SVGGElement;

  if (REDUCED.matches) {
    lockup.settled();
  } else {
    await lockup.play();
  }

  let busy = false;
  let asleep = false;
  let clicks = 0;
  let drowsy = 0;
  // Runs `move` unless another runs; the moves of a mascot never overlap.
  const alone = async (move: () => Promise<void>) => {
    if (busy) return;
    busy = true;
    try {
      await move();
    } finally {
      busy = false;
    }
  };

  const sleep = async () => {
    asleep = true;
    await alone(() => mascot.feel('sleep', 0.8));
    while (asleep) await mascot.breathe();
  };
  const rest = () => {
    clearTimeout(drowsy);
    drowsy = window.setTimeout(() => void sleep(), DROWSY * 1000);
  };
  const wake = () => {
    rest();
    if (!asleep) return;
    asleep = false;
    void after(0.1, () => alone(() => mascot.feel('calm', 0.4)));
  };

  body.addEventListener('pointerenter', () => void alone(() => mascot.feel('glee', 0.3)));
  body.addEventListener('pointerleave', () => void alone(() => mascot.feel('calm', 0.35)));
  body.addEventListener('click', () => {
    wake();
    void alone(async () => {
      await mascot.hop(HOP);
      await mascot.feel(CLICKED[clicks++ % CLICKED.length], 0.3);
      await wait(HOLD);
      await mascot.feel('glee', 0.3);
    });
  });
  word.addEventListener('pointerenter', () => void alone(() => lockup.land(0.5)));

  // The eyes follow the pointer, once a frame at most.
  let pointer: {x: number; y: number} | undefined;
  let queued = false;
  addEventListener(
    'pointermove',
    (event) => {
      wake();
      pointer = {x: event.clientX, y: event.clientY};
      if (queued || REDUCED.matches) return;
      queued = true;
      requestAnimationFrame(() => {
        queued = false;
        if (pointer === undefined || asleep) return;
        const box = body.getBoundingClientRect();
        const dx = pointer.x - (box.left + box.width / 2);
        const dy = pointer.y - (box.top + box.height / 2);
        const pull = Math.min(1, Math.hypot(dx, dy) / GAZE.reach);
        const angle = Math.atan2(dy, dx);
        mascot.gaze.to({x: Math.cos(angle) * pull * GAZE.x, y: Math.sin(angle) * pull * GAZE.y}, 0.25);
      });
    },
    {passive: true},
  );
  rest();

  // A blink now and then, while awake and still.
  if (REDUCED.matches) return;
  for (;;) {
    await wait(BLINK.least + Math.random() * BLINK.spread);
    if (!asleep) await alone(() => mascot.blink());
  }
}
