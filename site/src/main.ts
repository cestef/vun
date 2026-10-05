import '@fontsource-variable/jetbrains-mono';
import 'virtual:brand.css';
import './style.css';

import {chrome} from './chrome';
import {hero} from './hero';
import {icon} from './icons';

// The steps of installing: fetched, read, then run. The installer is this site's own file, so the
// address is the page's.
const INSTALLER = 'install.sh';

function steps(): string[] {
  const url = new URL(INSTALLER, location.origin + import.meta.env.BASE_URL);
  return [`curl -fsSLO ${url}`, `less ${INSTALLER}`, `sh ${INSTALLER}`];
}

function install() {
  const lines = steps();
  document.querySelector('[data-command]')!.replaceChildren(
    ...lines.map((line) => {
      const row = document.createElement('span');
      row.className = 'line';
      const prompt = Object.assign(document.createElement('span'), {className: 'prompt', textContent: '$ '});
      row.append(prompt, line);
      return row;
    }),
  );
  const copy = document.querySelector('[data-copy]') as HTMLButtonElement;
  copy.append(icon('copy'), icon('check'));
  let reset = 0;
  copy.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(lines.join('\n'));
      copy.classList.add('copied');
      copy.title = 'copied';
    } catch {
      copy.title = 'select the commands instead';
    }
    clearTimeout(reset);
    reset = window.setTimeout(() => {
      copy.classList.remove('copied');
      copy.title = 'copy';
    }, 1600);
  });
}

chrome();
install();
void hero(document.querySelector('[data-logo]') as HTMLElement);
