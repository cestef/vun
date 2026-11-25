// Every lesson's programs through the compiler as the playground runs them, so that a lesson keeps
// true as the language moves: the program a lesson starts with must not be done as it stands (a
// free one only has to run), and the answer must be.

import {describe, expect, test} from 'vitest';

import {judge} from '../src/play/lessons';
import {blocks, lessons} from './content';
import {load, play} from './play';

const toolchain = await load();

describe('lessons', () => {
  for (const lesson of lessons()) {
    test(lesson.id, async () => {
      const start = await play(toolchain, lesson.start);
      if (lesson.expect === undefined) {
        expect(start.built, 'a free lesson builds').toBe(true);
        expect(start.exit).toBe(0);
        return;
      }
      expect(judge(lesson.expect, start), 'it is not done before it starts').toBe(false);
      expect(judge(lesson.expect, await play(toolchain, lesson.answer)), 'its answer is done').toBe(true);
    });
  }
});

describe('guide', () => {
  for (const [i, block] of blocks().entries()) {
    test(`${block.page} #${i + 1} (${block.check})`, async () => {
      const outcome = await play(toolchain, block.code);
      if (block.check === 'run') expect([outcome.built, outcome.exit]).toEqual([true, 0]);
      else expect(outcome.built).toBe(false);
    });
  }
});
