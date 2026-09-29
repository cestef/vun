// The mockup episode, a clip: the README's tour run and changed.

import type {Episode} from '../../clip/episode';
import {PACK, SESSION, SPEAKER, STATS, TOUR, URL} from './data';

export const EPISODE: Episode = {
  series: 'devlog',
  number: 1,
  title: 'A tour of Vun',
  speaker: SPEAKER,
  url: URL,
  segments: [
    {kind: 'title'},
    {kind: 'code', source: TOUR, caption: 'Integers are ranges, and types are values known at compile time.'},
    {kind: 'terminal', session: SESSION, caption: 'It builds and runs; the C and native backends agree.'},
    {...PACK, caption: 'A recent change: print takes its arguments as a pack.'},
    {kind: 'stats', stats: STATS, caption: 'Where the compiler stands.'},
    {kind: 'end'},
  ],
};
