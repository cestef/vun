// The hero's lockup and what it does when played with: the mascot follows the pointer with its
// eyes, cheers when pointed at, hops into another mood when clicked, and falls asleep when left
// alone; pointing at the word makes it land again.

import type {MoodName} from '../../assets/motion/src/mascot/moods';
import {Lockup} from './lockup';
import {Latest, ease, visible, wait} from './motion';
import {REDUCED} from './svg';

// The moods a click shows, in turn.
const CLICKED: MoodName[] = ['wink', 'awe', 'cross', 'sad', 'glee'];
// How far the eyes follow the pointer, in units of the view box, and how far from the mascot it
// is followed at most, in pixels.
const GAZE = {x: 8, y: 5, reach: 500, follow: 0.12};
// How long a clicked mood holds, and how long without the pointer until it sleeps, in seconds.
const HOLD = 0.9;
// How long a mood takes to show, in seconds.
const FEEL = 0.18;
const DROWSY = 25;
// How long between blinks when idle, in seconds: at least `least`, up to `spread` more.
const BLINK = {least: 3, spread: 4};
// How high a click makes it hop, in units of the view box.
const HOP = 45;
// The class the hero's text waits under until the mascot has landed, and the one that ends the wait.
const STAGED = 'staged';
// The mood a poke settles into, as pointing at it does.
const HOLDING: MoodName = 'glee';
const ARRIVED = 'arrived';

export async function hero(slot: HTMLElement) {
  const lockup = new Lockup();
  slot.append(lockup.svg);
  const {mascot} = lockup;
  const body = mascot.root.group;
  const page = slot.closest('.hero');

  if (REDUCED.matches) {
    lockup.settled();
  } else {
    page?.classList.add(STAGED);
    // A press or a key during the sting ends it.
    const playing = new AbortController();
    const skip = () => lockup.skip();
    addEventListener('pointerdown', skip, {signal: playing.signal});
    addEventListener('keydown', skip, {signal: playing.signal});
    await lockup.play();
    playing.abort();
  }
  page?.classList.add(ARRIVED);

  let asleep = false;
  let clicks = 0;
  let drowsy = 0;
  // The mascot does one thing at a time and the newest input wins: a move begins by ending the one
  // before, and asks `live` between its steps whether it still is the one.
  const moves = new Latest();
  let moving = false;
  const play = async (move: (live: () => boolean) => Promise<unknown>, settle = true) => {
    const live = moves.begin();
    moving = true;
    if (settle) void mascot.settle();
    try {
      await move(live);
    } finally {
      if (live()) moving = false;
    }
  };

  const sleep = async () => {
    asleep = true;
    await play(() => mascot.feel('sleep', 0.8));
    while (asleep) {
      await visible();
      if (!moving) await mascot.breathe();
      else await wait(0.2);
    }
  };
  const rest = () => {
    clearTimeout(drowsy);
    drowsy = window.setTimeout(() => void sleep(), DROWSY * 1000);
  };
  const wake = () => {
    rest();
    if (!asleep) return;
    asleep = false;
    void play(() => mascot.feel('calm', 0.3));
  };

  // A touch has no hover: its tap is the poke. A poke is not cut short by the pointer leaving, as
  // its hop moves the mascot from under it; the mood it ends in is the pointer's.
  const hovering = (event: PointerEvent) => event.pointerType !== 'touch';
  let over = false;
  let poking = false;
  const mood = (): MoodName => (over ? HOLDING : 'calm');
  body.addEventListener('pointerenter', (event) => {
    over = hovering(event);
    if (over && !poking) void play(() => mascot.feel(mood(), FEEL, false));
  });
  body.addEventListener('pointerleave', (event) => {
    over = false;
    if (hovering(event) && !poking) void play(() => mascot.feel(mood(), FEEL, false));
  });
  const poke = () => {
    wake();
    // Pressed again in the air, it changes mood and does not hop again.
    const flying = mascot.hopping;
    void play(async (live) => {
      poking = true;
      try {
        if (!flying && (!(await mascot.hop(HOP)) || !live())) return;
        if (!(await mascot.feel(CLICKED[clicks++ % CLICKED.length], FEEL, false)) || !(await wait(HOLD)) || !live()) return;
        await mascot.feel(mood(), FEEL, false);
      } finally {
        if (live()) poking = false;
      }
    }, !flying);
  };
  // On the press, not the release.
  body.addEventListener('pointerdown', poke);
  body.addEventListener('keydown', (event) => {
    if (event.key !== 'Enter' && event.key !== ' ') return;
    event.preventDefault();
    poke();
  });
  body.setAttribute('tabindex', '0');
  body.setAttribute('role', 'button');
  body.setAttribute('aria-label', 'poke the mascot');
  lockup.hit.addEventListener('pointerenter', (event) => hovering(event) && void lockup.land(0.5));
  lockup.hit.addEventListener('pointerdown', () => void lockup.land(0.8));

  // The eyes follow the pointer, once a frame at most.
  let pointer: {x: number; y: number} | undefined;
  let queued = false;
  // Where the mascot's middle is on the page, read when the page moves under it, not at each frame.
  let middle = {x: 0, y: 0};
  const measure = () => {
    const box = body.getBoundingClientRect();
    middle = {x: box.left + box.width / 2, y: box.top + box.height / 2};
  };
  measure();
  addEventListener('resize', measure, {passive: true});
  addEventListener('scroll', measure, {passive: true});
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
        const dx = pointer.x - middle.x;
        const dy = pointer.y - middle.y;
        const pull = Math.min(1, Math.hypot(dx, dy) / GAZE.reach);
        const angle = Math.atan2(dy, dx);
        mascot.gaze.to({x: Math.cos(angle) * pull * GAZE.x, y: Math.sin(angle) * pull * GAZE.y}, GAZE.follow, ease.outCubic);
      });
    },
    {passive: true},
  );
  rest();

  // A blink now and then, while awake and still.
  if (REDUCED.matches) return;
  for (;;) {
    await wait(BLINK.least + Math.random() * BLINK.spread);
    await visible();
    if (!asleep && !moving) await mascot.blink();
  }
}
