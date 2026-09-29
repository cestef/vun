// The mascot's moods: a face each, and how its body carries it.

import {type Eye, type Face, type Mouth, mirror} from './shape.ts';

const EYE = {
  open: {from: {x: 0, y: -7}, bend: {x: 0, y: 0}, to: {x: 0, y: 7}},
  wide: {from: {x: 0, y: -10}, bend: {x: 0, y: 0}, to: {x: 0, y: 10}},
  // Closed in a smile, arched up.
  glad: {from: {x: -10, y: 4}, bend: {x: 0, y: -4}, to: {x: 10, y: 4}},
  // Closed and hanging.
  shut: {from: {x: -10, y: 0}, bend: {x: 0, y: 4}, to: {x: 10, y: 0}},
  // Slanting down towards the middle.
  cross: {from: {x: -9, y: -4}, bend: {x: 0, y: 0}, to: {x: 9, y: 4}},
} satisfies Record<string, Eye>;

const MOUTH = {
  smile: {width: 26, top: 6, bottom: 6},
  grin: {width: 28, top: 0, bottom: 18},
  round: {width: 8, top: -7, bottom: 7},
  frown: {width: 22, top: -5, bottom: -5},
  flat: {width: 18, top: 0, bottom: 0},
  small: {width: 8, top: 0, bottom: 3},
} satisfies Record<string, Mouth>;

// How the body stands: stretched by `stretch` (wider above one, taller below), leaning by
// `lean` degrees.
export interface Pose {
  stretch: number;
  lean: number;
}

export interface Mood {
  face: Face;
  pose: Pose;
}

const still = {x: 0, y: 0};
const both = (eye: Eye): [Eye, Eye] => [eye, mirror(eye)];

export const MOODS = {
  calm: {face: {eyes: both(EYE.open), mouth: MOUTH.smile, look: still}, pose: {stretch: 1, lean: 0}},
  glee: {face: {eyes: both(EYE.glad), mouth: MOUTH.grin, look: still}, pose: {stretch: 0.97, lean: 0}},
  wink: {
    face: {eyes: [EYE.open, mirror(EYE.glad)], mouth: MOUTH.grin, look: {x: 2, y: 0}},
    pose: {stretch: 1, lean: 3},
  },
  awe: {face: {eyes: both(EYE.wide), mouth: MOUTH.round, look: {x: 0, y: -4}}, pose: {stretch: 0.94, lean: 0}},
  sad: {face: {eyes: both(EYE.open), mouth: MOUTH.frown, look: {x: 0, y: 6}}, pose: {stretch: 1.05, lean: 0}},
  cross: {face: {eyes: both(EYE.cross), mouth: MOUTH.flat, look: still}, pose: {stretch: 1.02, lean: 0}},
  sleep: {face: {eyes: both(EYE.shut), mouth: MOUTH.small, look: {x: 0, y: 4}}, pose: {stretch: 1.03, lean: 0}},
} satisfies Record<string, Mood>;

export type MoodName = keyof typeof MOODS;
