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
export const tidy = (text: string) => text.replace(/[ \t]+$/gm, '').trim();

// How one thing a lesson asks stands after a run: not asked of yet, met, or missed, and what the
// run did instead.
export type Standing = 'pending' | 'met' | 'missed';

export interface Verdict {
  // What is asked, as a line.
  name: string;
  standing: Standing;
  // What the run did instead, when it missed.
  found?: string;
  // For output of several lines, how each line stands, so that a long output is one row and a strip.
  cells?: Standing[];
}

const quoted = (text: string) => `\`${text}\``;

// Each thing `expect` asks, judged by `outcome`; every one is pending before a run.
export function verdicts(expect: Expect, outcome: Outcome | undefined): Verdict[] {
  const judged = (name: string, ok: boolean | undefined, found?: string): Verdict => ({
    name,
    standing: ok === undefined ? 'pending' : ok ? 'met' : 'missed',
    ...(ok === false && found !== undefined ? {found} : {}),
  });
  if (expect.error !== undefined) {
    const refused = outcome === undefined ? undefined : !outcome.built;
    const ids = outcome?.diagnostics.filter((d) => d.severity === 'error').map((d) => d.id) ?? [];
    return [
      judged('the compiler refuses the program', refused, 'it built'),
      judged(`with ${quoted(expect.error)}`, refused === false ? undefined : outcome && ids.includes(expect.error), ids.length > 0 ? `it said ${ids.map(quoted).join(', ')}` : 'it said nothing'),
    ];
  }
  const built = outcome?.built;
  const ends = expect.exit ?? 0;
  const ended = outcome === undefined || !outcome.built ? undefined : outcome.trap === undefined && outcome.exit === ends;
  const list = [
    judged('it builds', built, `${outcome?.diagnostics.filter((d) => d.severity === 'error').length ?? 0} error(s)`),
    judged(ends === 0 ? 'it ends normally' : `it exits with ${ends}`, ended, outcome?.trap === undefined ? `it exited with ${outcome?.exit}` : `it trapped: ${outcome.trap}`),
  ];
  if (expect.output === undefined) return list;
  const wanted = tidy(expect.output).split('\n');
  const got = outcome === undefined || !outcome.built ? undefined : tidy(outcome.output).split('\n').filter((line, i, all) => line !== '' || all.length > 1);
  const name = wanted.length === 1 ? `prints ${quoted(wanted[0])}` : `prints the ${wanted.length} lines asked for`;
  if (got === undefined) return [...list, {name, standing: 'pending', ...(wanted.length > 1 ? {cells: wanted.map((): Standing => 'pending')} : {})}];
  const first = wanted.findIndex((line, i) => got[i] !== line);
  const extra = got.length - wanted.length;
  const ok = first < 0 && extra <= 0;
  const found =
    first >= 0
      ? got[first] === undefined
        ? `line ${first + 1}: nothing was printed, not ${quoted(wanted[first])}`
        : `line ${first + 1}: it printed ${quoted(got[first])}, not ${quoted(wanted[first])}`
      : `${extra} extra line(s) after the last one asked for`;
  const cells = wanted.length === 1 ? undefined : [...wanted.map((line, i): Standing => (got[i] === line ? 'met' : 'missed')), ...Array.from({length: Math.max(0, extra)}, (): Standing => 'missed')];
  return [...list, {name, standing: ok ? 'met' : 'missed', ...(ok ? {} : {found}), ...(cells === undefined ? {} : {cells})}];
}

export function judge(expect: Expect, outcome: Outcome): boolean {
  return verdicts(expect, outcome).every((verdict) => verdict.standing === 'met');
}
