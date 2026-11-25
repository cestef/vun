// The playground: the workbench with the whole page, starting from the text it was left with, or
// a shared link's, or the free program of the course.

import {render} from '@solidjs/web';

import {href} from '../site';
import {Action} from '../ui/Keycaps';
import {Cell} from '../ui/cell';
import {Keys} from '../ui/keys';
import free from '../content/docs/docs/learn/sandbox.vun?raw';
import {Bench} from './bench';
import {decode, encode} from './share';
import {keep, kept} from './store';
import {loadToolchain} from './toolchain';
import {Workbench} from './Workbench';

const KEPT = 'vun.play.code.sandbox';
// How long the share button says what it did, in milliseconds.
const SHOWN = 1600;

function Playground() {
  const keys = new Keys();
  const bench = new Bench(keys, loadToolchain(href('')), {
    code: decode(location.hash.slice(1)) ?? kept(KEPT) ?? free,
    changed: (code) => keep(KEPT, code),
    ended: () => undefined,
  });
  const label = new Cell('Share');
  let reset = 0;
  keys.on('share', async () => {
    const link = `${location.origin}${location.pathname}#${encode(bench.editor.code)}`;
    history.replaceState(null, '', link);
    try {
      await navigator.clipboard.writeText(link);
      label.value = 'Link copied';
    } catch {
      label.value = 'In the address bar';
    }
    clearTimeout(reset);
    reset = window.setTimeout(() => (label.value = 'Share'), SHOWN);
  });
  return (
    <Workbench
      bench={bench}
      tools={
        <Action keys={keys} action="share" class="tool" icon="link">
          {label.value}
        </Action>
      }
    />
  );
}

export function mountPlayground(root: HTMLElement) {
  render(() => <Playground />, root);
}
