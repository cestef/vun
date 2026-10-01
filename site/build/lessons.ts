// The guide's lessons (`site/lessons`), read for the playground and for the check that runs them.

import {parse} from 'smol-toml';

import type {Expect, Lesson} from '../src/play/lessons.ts';
import {source} from './source.ts';

const DIR = 'site/lessons';

interface Entry {
  id: string;
  title: string;
  text: string;
  task?: string;
  hint?: string;
  expect?: Expect;
}

export function lessons(): Lesson[] {
  const {lesson} = parse(source(`${DIR}/lessons.toml`)) as unknown as {lesson: Entry[]};
  return lesson.map((entry) => ({
    id: entry.id,
    title: entry.title,
    text: entry.text,
    task: entry.task ?? '',
    hint: entry.hint ?? '',
    start: source(`${DIR}/${entry.id}.vun`),
    answer: entry.expect === undefined ? '' : source(`${DIR}/${entry.id}.answer.vun`),
    expect: entry.expect,
  }));
}
