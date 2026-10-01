// The lessons of the guide, and whether a run did what one asks. A lesson is a program to read,
// change and run; it is done when a run ends as `expect` says, so the page and the check that
// keeps the lessons true to the compiler judge alike.

import type {Diagnostic} from './protocol.ts';

// How a run must end: the compiler refusing with the diagnostic `error`, or the program built and
// ending with `exit` (0 where unsaid), having written `output` if that is said.
export interface Expect {
  output?: string;
  error?: string;
  exit?: number;
}

export interface Lesson {
  id: string;
  title: string;
  // What the lesson shows, in Markdown: a few sentences.
  text: string;
  // What to do, one line.
  task: string;
  // Where to look, a nudge short of the answer.
  hint: string;
  // The program as the lesson starts, and as it ends. A lesson without an `expect` is free play:
  // it has no task, and nothing to answer.
  start: string;
  answer: string;
  expect: Expect | undefined;
}

// What a run did: the build, then the program if it was built.
export interface Outcome {
  built: boolean;
  diagnostics: Diagnostic[];
  // What the program wrote, and how it ended: its exit code, or the trap that ended it.
  output: string;
  exit: number | undefined;
  trap: string | undefined;
  // What the compiler wrote that was not a record.
  notes: string;
}

// Lines without the space at their ends, and the text without the lines at its ends.
const tidy = (text: string) => text.replace(/[ \t]+$/gm, '').trim();

export function judge(expect: Expect, outcome: Outcome): boolean {
  if (expect.error !== undefined) return !outcome.built && outcome.diagnostics.some((d) => d.severity === 'error' && d.id === expect.error);
  if (!outcome.built || outcome.trap !== undefined || outcome.exit !== (expect.exit ?? 0)) return false;
  return expect.output === undefined || tidy(outcome.output) === tidy(expect.output);
}
