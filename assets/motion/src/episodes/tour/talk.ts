// The mockup talk: the tour, as slides.

import type {Episode} from '../../clip/episode';
import {PACK, PIPELINE, SESSION, SPEAKER, STATS, TOUR, URL} from './data';

export const TALK: Episode = {
  series: 'talk',
  number: 1,
  title: 'A tour of Vun',
  speaker: SPEAKER,
  url: URL,
  segments: [
    {kind: 'title'},
    {
      kind: 'callout',
      text: 'What is available to std is available to you.',
      source: 'docs/spec/00-principles.md',
    },
    {kind: 'chapter', title: 'The language'},
    {kind: 'code', source: TOUR},
    {kind: 'split', source: TOUR, session: SESSION},
    {...PACK},
    {kind: 'chapter', title: 'The compiler'},
    {...PIPELINE, title: 'Every stage has a dump'},
    {
      kind: 'points',
      title: 'Built',
      points: [
        'C, wasm and native backends: x86-64, arm64, Thumb-2',
        'ELF and Mach-O objects, linked by gnu, mold or wild',
        'A persisted, parallel query engine',
      ],
    },
    {kind: 'stats', stats: STATS},
    {kind: 'end'},
  ],
};
