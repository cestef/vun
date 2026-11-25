// The guide's content as the tests read it: the lessons beside their programs, and the blocks of
// code the guide asks the compiler to run.

import {readFileSync, readdirSync} from 'node:fs';

import MarkdownIt from 'markdown-it';
import {parse} from 'yaml';

import type {Expect} from '../src/play/lessons';

const DOCS = 'src/content/docs/docs';
const LEARN = `${DOCS}/learn`;

export interface Lesson {
  id: string;
  expect: Expect | undefined;
  start: string;
  answer: string;
}

const frontmatter = (text: string) => parse(/^---\n([\s\S]*?)\n---/.exec(text)?.[1] ?? '') as {lesson?: {expect?: Expect}};

// The lessons that have an editor, in the order of the course.
export function lessons(): Lesson[] {
  return readdirSync(LEARN)
    .filter((name) => name.endsWith('.md'))
    .map((name) => name.slice(0, -'.md'.length))
    .map((id) => ({id, data: frontmatter(readFileSync(`${LEARN}/${id}.md`, 'utf8'))}))
    .filter(({data}) => data.lesson !== undefined)
    .map(({id, data}) => ({
      id,
      expect: data.lesson?.expect,
      start: readFileSync(`${LEARN}/${id}.vun`, 'utf8'),
      answer: data.lesson?.expect === undefined ? '' : readFileSync(`${LEARN}/${id}.answer.vun`, 'utf8'),
    }));
}

// What the compiler is asked of a block: `run` ends normally, `error` is refused.
export type Check = 'run' | 'error';

export interface Block {
  page: string;
  check: Check;
  code: string;
}

// The blocks of the guide's pages tagged `vun run` or `vun error`.
export function blocks(): Block[] {
  const markdown = new MarkdownIt();
  return readdirSync(DOCS)
    .filter((name) => /\.mdx?$/.test(name))
    .flatMap((name) =>
      markdown
        .parse(readFileSync(`${DOCS}/${name}`, 'utf8'), {})
        .filter((token) => token.type === 'fence')
        .map((token) => ({token, info: token.info.trim().split(/\s+/)}))
        .filter(({info}) => info[0] === 'vun' && (info[1] === 'run' || info[1] === 'error'))
        .map(({token, info}) => ({page: name, check: info[1] as Check, code: token.content})),
    );
}
