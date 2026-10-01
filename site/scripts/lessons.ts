// Runs every lesson's programs through the compiler as the playground does, so that a lesson keeps
// true as the language moves: the program a lesson starts with must not be done as it stands (a
// free one only has to run), and the answer must be. `node scripts/lessons.ts` checks them all,
// `node scripts/lessons.ts errors` one, showing what each run reported.

import {lessons} from '../build/lessons.ts';
import {type Outcome, judge} from '../src/play/lessons.ts';
import {load, play} from './playground.ts';

const only = process.argv[2];
const toolchain = await load();

const say = (m: {parts: {value: string}[]}) => m.parts.map((part) => part.value).join('');

function describe(outcome: Outcome): string {
  const lines = outcome.diagnostics.map((d) => `    ${d.severity} ${d.id}: ${say(d.message)}`);
  if (outcome.notes !== '') lines.push(...outcome.notes.trimEnd().split('\n').map((line) => `    ${line}`));
  lines.push(outcome.built ? `    exit ${outcome.exit}${outcome.trap === undefined ? '' : `, trap ${outcome.trap}`}` : '    not built');
  if (outcome.output !== '') lines.push(...outcome.output.trimEnd().split('\n').map((line) => `    | ${line}`));
  return lines.join('\n');
}

let wrong = 0;
for (const lesson of lessons().filter((l) => only === undefined || l.id === only)) {
  const problems: string[] = [];
  const start = await play(toolchain, lesson.start);
  if (lesson.expect === undefined) {
    if (!start.built || start.exit !== 0) problems.push(`its program does not run:\n${describe(start)}`);
  } else {
    if (judge(lesson.expect, start)) problems.push(`it is done before it starts:\n${describe(start)}`);
    const answer = await play(toolchain, lesson.answer);
    if (!judge(lesson.expect, answer)) problems.push(`its answer is not done:\n${describe(answer)}`);
  }
  console.log(`${problems.length === 0 ? 'ok  ' : 'FAIL'} ${lesson.id}`);
  for (const problem of problems) console.log(`  ${problem}`);
  if (only !== undefined && problems.length === 0) console.log(describe(start));
  wrong += problems.length === 0 ? 0 : 1;
}
if (wrong > 0) {
  console.error(`${wrong} lesson${wrong === 1 ? '' : 's'} to fix`);
  process.exit(1);
}
