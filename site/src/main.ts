import '@fontsource-variable/jetbrains-mono';
import 'virtual:brand.css';
import './style.css';

import {chrome} from './chrome';
import {hero} from './hero';

const INSTALL = 'curl -fsSL https://raw.githubusercontent.com/cestef/vun/main/install.sh | sh';

function install() {
  document.querySelector('[data-command]')!.textContent = INSTALL;
  const copy = document.querySelector('[data-copy]') as HTMLButtonElement;
  let reset = 0;
  copy.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(INSTALL);
      copy.textContent = 'copied';
    } catch {
      copy.textContent = 'select it';
    }
    clearTimeout(reset);
    reset = window.setTimeout(() => (copy.textContent = 'copy'), 1600);
  });
}

chrome();
install();
void hero(document.querySelector('[data-logo]') as HTMLElement);
