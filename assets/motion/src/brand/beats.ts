// When things happen in the stings, in seconds: the scenes move by these, and `pnpm sound`
// places the sound mark by them, so sight and sound agree.

// The wordmark traced (`Wordmark.draw` at its own pace): the v, then the rest after a moment,
// each pen lifting at its time.
export const TRACE = {
  pen: 0.15,
  v: 0.5,
  un: {after: 0.35, length: 0.9},
  lift: {v: 0.45, un: 1.2, length: 0.2},
};
export const TRACED = Math.max(TRACE.v, TRACE.un.after + TRACE.un.length, TRACE.lift.un + TRACE.lift.length);

// The mascot dropping in beside the word (`Wordmark.welcome`): a moment, then its fall.
export const FALL = {after: 0.3, length: 0.45};

// The sting: when the word is built after the grid opens, and how long after the strokes are
// traced it lands and the mascot is welcomed.
export const STING = {build: 1, land: 0.35, welcome: 1.1};
export const STING_LAND = STING.build + TRACED + STING.land;
export const STING_ARRIVAL = STING.build + TRACED + STING.welcome + FALL.after + FALL.length;

// The short sting: when the tracing starts, how fast, and how long after the word lands the
// mascot is welcomed.
export const SHORT = {start: 0.2, pace: 0.5, welcome: 0.5};
export const SHORT_LAND = SHORT.start + TRACED * SHORT.pace;
export const SHORT_ARRIVAL = SHORT_LAND + SHORT.welcome + FALL.after + FALL.length;
