// What the tour's clip and talk show, from the repository: the README's program, the session and
// the change `pnpm record tour` captured, the pipeline diagram of the cards.

import program from '../../../../../corpus/accept/programs/tour.vun?raw';
import pipeline from '../../../../cards/diagrams/pipeline/dark.svg?url';
import type {Segment, Session, Source, Speaker, Stat} from '../../clip/episode';
import changes from './changes.json';
import recorded from './session.json';

// A program as a card shows it: without its module doc, which says why it is in the corpus.
const shown = (text: string) =>
  text
    .split('\n')
    .filter((line) => !line.startsWith('//!'))
    .join('\n')
    .trim();

export const SPEAKER: Speaker = {name: 'cestef', role: 'author of Vun'};
export const URL = 'github.com/cestef/vun';

export const TOUR: Source = {
  file: 'tour.vun',
  text: shown(program),
  focus: [
    [4, 5],
    [7, 8],
  ],
};

export const SESSION = recorded as Session;

const [pack] = changes;
export const PACK = {
  kind: 'diff',
  file: pack.file.split('/').at(-1) ?? pack.file,
  before: shown(pack.before),
  after: shown(pack.after),
} satisfies Segment;

// As `docs/HANDOFF.md` states them on 2026-09-29.
export const STATS: Stat[] = [
  {value: 650, label: 'tests passing'},
  {value: 295, label: 'corpus programs'},
  {value: 3, label: 'native targets'},
];

export const PIPELINE = {kind: 'image', src: pipeline} satisfies Segment;
