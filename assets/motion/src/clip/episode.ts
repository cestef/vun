// What a clip or a talk shows: an episode, a sequence of segments, each drawn by the scene of its
// kind. The project gives it to the scenes through its variables (`project.ts`).

// The styles of vun's output (`Style` of `vun-term`), as `--format styled` names them.
export type Style =
  | 'heading'
  | 'literal'
  | 'placeholder'
  | 'good'
  | 'error'
  | 'warning'
  | 'note'
  | 'help'
  | 'accent'
  | 'scope'
  | 'key'
  | 'id'
  | 'dim'
  | 'strong';

export interface Piece {
  style?: Style;
  text: string;
}

// A line of output as people read it.
export interface Line {
  pieces: Piece[];
}

export interface Step {
  // The program and its arguments.
  command: string[];
  output: Line[];
}

// Where commands ran and what they printed, as `pnpm record` writes it.
export interface Session {
  directory: string;
  steps: Step[];
}

export interface Source {
  file: string;
  text: string;
  // Ranges of lines to point at in turn, both ends included, counted from 1.
  focus?: [number, number][];
}

export interface Speaker {
  name: string;
  role: string;
}

// A number counted up to, what it counts after it.
export interface Stat {
  value: number;
  label: string;
}

// What each kind of segment shows; `caption` is said under it in a clip, for watching without
// sound.
export type Segment = {caption?: string} & (
  | {kind: 'title'}
  | {kind: 'chapter'; title: string}
  | {kind: 'points'; title: string; points: string[]}
  | {kind: 'code'; source: Source}
  | {kind: 'diff'; file: string; before: string; after: string}
  | {kind: 'terminal'; session: Session}
  | {kind: 'split'; source: Source; session: Session}
  | {kind: 'callout'; text: string; source?: string}
  | {kind: 'stats'; stats: Stat[]}
  | {kind: 'image'; title?: string; src: string}
  | {kind: 'end'}
);

export type Kind = Segment['kind'];

// A segment, and where it stands among the others.
export interface Placed {
  segment: Segment;
  index: number;
  count: number;
}

export interface Episode {
  series: string;
  number: number;
  title: string;
  speaker: Speaker;
  url: string;
  segments: Segment[];
}
