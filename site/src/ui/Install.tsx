// The steps of installing: fetched, read, then run. The installer is this site's own file, so the
// address is the page's, which only the page knows.

import {render} from '@solidjs/web';
import {For} from 'solid-js';

import {href} from '../site';
import {Cell} from './cell';
import {Icon} from './Icon';

const INSTALLER = 'install.sh';
// How long the copy button shows that it copied, in milliseconds.
const COPIED = 1600;

const steps = () => [`curl -fsSLO ${new URL(href(INSTALLER), location.origin)}`, `less ${INSTALLER}`, `sh ${INSTALLER}`];

function Install() {
  const lines = steps();
  const copied = new Cell(false);
  const title = new Cell('copy');
  let reset = 0;
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(lines.join('\n'));
      copied.value = true;
      title.value = 'copied';
    } catch {
      title.value = 'select the commands instead';
    }
    clearTimeout(reset);
    reset = window.setTimeout(() => {
      copied.value = false;
      title.value = 'copy';
    }, COPIED);
  };
  return (
    <div class="install">
      <code>
        <For each={lines}>
          {(line) => (
            <span class="line">
              <span class="prompt">$ </span>
              {line}
            </span>
          )}
        </For>
      </code>
      <button type="button" class={{copied: copied.value}} aria-label="copy the commands" title={title.value} onClick={copy}>
        <Icon name={copied.value ? 'check' : 'copy'} />
      </button>
    </div>
  );
}

export function mountInstall(root: HTMLElement) {
  render(() => <Install />, root);
}
