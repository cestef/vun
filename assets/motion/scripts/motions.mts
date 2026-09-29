// The mascot's CSS animations: which moods each shows, how much room it needs above the body
// (a share of its height) and the rules that move it. The parts they move are those of
// `src/mascot/svg.ts`; `transform-box: fill-box` makes each turn on its own box.

import {MOODS, type MoodName} from '../src/mascot/moods.ts';
import {document, extent, mascot} from '../src/mascot/svg.ts';

export interface Motion {
  moods: MoodName[];
  headroom: number;
  css: string;
}

// Every mood in turn, this long each, a blink hiding each change.
const ALL = Object.keys(MOODS) as MoodName[];
const TURN = 2;
const ROUND = ALL.length * TURN;

export const MOTIONS = {
  // A double blink every few seconds.
  blink: {
    moods: ['calm'],
    headroom: 0,
    css: `.eye { transform-box: fill-box; transform-origin: center; animation: blink 4s infinite; }
    @keyframes blink {
      0%, 86%, 90%, 94%, 100% { transform: scaleY(1); }
      88%, 92% { transform: scaleY(0.1); }
    }`,
  },
  // Looks left, then right, then back.
  glance: {
    moods: ['calm'],
    headroom: 0,
    css: `.face { animation: glance 4s ease-in-out infinite; }
    .eye { transform-box: fill-box; transform-origin: center; animation: blink 4s infinite; }
    @keyframes glance {
      0%, 15%, 85%, 100% { transform: translate(0, 0); }
      25%, 40% { transform: translate(-7px, 0); }
      55%, 70% { transform: translate(7px, -2px); }
    }
    @keyframes blink {
      0%, 46%, 50%, 100% { transform: scaleY(1); }
      48% { transform: scaleY(0.1); }
    }`,
  },
  // Crouches, jumps stretched, lands squashed.
  hop: {
    moods: ['glee'],
    headroom: 0.4,
    css: `.mascot { transform-box: fill-box; transform-origin: 50% 100%; animation: hop 1.3s infinite; }
    @keyframes hop {
      0%, 88%, 100% { transform: translateY(0) scale(1, 1); }
      10% { transform: translateY(0) scale(1.12, 0.88); animation-timing-function: ease-out; }
      32% { transform: translateY(-48%) scale(0.94, 1.07); }
      42% { transform: translateY(-54%) scale(1, 1); animation-timing-function: ease-in; }
      60% { transform: translateY(0) scale(0.96, 1.05); }
      68% { transform: translateY(0) scale(1.1, 0.9); }
      78% { transform: translateY(0) scale(0.98, 1.02); }
    }`,
  },
  // Swells and settles, asleep.
  breathe: {
    moods: ['sleep'],
    headroom: 0,
    css: `.mascot { transform-box: fill-box; transform-origin: 50% 100%; animation: breathe 3.6s ease-in-out infinite; }
    @keyframes breathe {
      0%, 100% { transform: scale(1, 1); }
      50% { transform: scale(1.04, 0.96); }
    }`,
  },
  // Holds still, then shakes.
  shake: {
    moods: ['cross'],
    headroom: 0,
    css: `.mascot { animation: shake 2.2s infinite; }
    @keyframes shake {
      0%, 60%, 100% { transform: translate(0, 0); }
      64%, 72%, 80% { transform: translate(-4px, 0); }
      68%, 76%, 84% { transform: translate(4px, 0); }
    }`,
  },
  // Every mood in turn.
  moods: {
    moods: ALL,
    headroom: 0,
    css: `.mood { opacity: 0; animation: turn ${ROUND}s step-end infinite; }
    ${ALL.map((_, i) => `.mood:nth-child(${i + 1}) { animation-delay: ${i * TURN - ROUND}s; }`).join('\n    ')}
    .eye { transform-box: fill-box; transform-origin: center; animation: change ${TURN}s infinite; }
    @keyframes turn {
      0% { opacity: 1; }
      ${(100 / ALL.length).toFixed(3)}% { opacity: 0; }
    }
    @keyframes change {
      0%, 100% { transform: scaleY(0.1); }
      8%, 94% { transform: scaleY(1); }
    }`,
  },
} satisfies Record<string, Motion>;

// Still for those who ask for less motion: the first mood, held.
export const REDUCED = `@media (prefers-reduced-motion: reduce) {
      * { animation: none !important; }
      .mood:first-child { opacity: 1; }
    }`;

// Room either side of an animation's frame, a share of the body's width, for it to squash and
// shake in.
const SIDES = 0.1;

// The SVG of `motion`, painted by the rules of `style`, framed with its headroom and sides.
export function animated(motion: Motion, style: string): string {
  const box = extent();
  const above = box.height * motion.headroom;
  const side = box.width * SIDES;
  const framed = {x: box.x - side, y: box.y - above, width: box.width + 2 * side, height: box.height + above};
  const drawn = mascot(...motion.moods.map((mood) => MOODS[mood].face));
  return document(framed, `${style}\n    ${motion.css}\n    ${REDUCED}`, drawn);
}
