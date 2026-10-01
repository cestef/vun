// Builds and runs a file the way the playground does and tells what it would show:
// `node scripts/try.ts program.vun`.

import {readFileSync} from 'node:fs';

import {load, play} from './playground.ts';

const outcome = await play(await load(), readFileSync(process.argv[2], 'utf8'));
for (const d of outcome.diagnostics) {
  const at = d.primary === null ? '' : ` at ${d.primary.at.start.line}:${d.primary.at.start.column}`;
  const say = (m: {parts: {value: string}[]}) => m.parts.map((part) => part.value).join('');
  console.log(`${d.severity} ${d.id}${at}: ${say(d.message)}`);
  for (const label of d.labels) if (label.message !== null) console.log(`    label: ${say(label.message)}`);
  for (const child of d.children) console.log(`    ${child.kind}: ${say(child.message)}`);
  for (const fix of d.fixes) console.log(`    fix: ${say(fix.message)}`);
}
if (outcome.notes !== '') console.log(outcome.notes.trimEnd());
console.log(outcome.built ? `built; exit ${outcome.exit}${outcome.trap === undefined ? '' : `, trap ${outcome.trap}`}` : `not built${outcome.trap === undefined ? '' : `, trap ${outcome.trap}`}`);
if (outcome.output !== '') console.log(outcome.output.trimEnd());
