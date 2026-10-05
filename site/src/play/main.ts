import '@fontsource-variable/jetbrains-mono';
import 'virtual:brand.css';
import '../style.css';
import './workbench.css';
import './play.css';

import lessons from 'virtual:lessons';

import {chrome} from '../chrome';
import {Keys} from '../keys';
import {decode, encode} from './share';
import {keep, kept} from './store';
import {loadToolchain} from './toolchain';
import {Workspace} from './workspace';

// The playground starts from the text it was left with, or a shared link's, or the course's free
// program, which is the last lesson.
const free = lessons[lessons.length - 1];
const KEPT = `vun.play.code.${free.id}`;

const root = document.querySelector('[data-workspace]') as HTMLElement;
const keys = new Keys(root.ownerDocument);
const workspace = new Workspace(root, loadToolchain(import.meta.env.BASE_URL), keys, {
  code: decode(location.hash.slice(1)) ?? kept(KEPT) ?? free.start,
  changed: (code) => keep(KEPT, code),
  ended: () => undefined,
});

const label = root.querySelector('[data-share-label]') as HTMLElement;
let shared = 0;
keys.on('share', async () => {
  const link = `${location.origin}${location.pathname}#${encode(workspace.editor.code)}`;
  history.replaceState(null, '', link);
  try {
    await navigator.clipboard.writeText(link);
    label.textContent = 'Link copied';
  } catch {
    label.textContent = 'In the address bar';
  }
  clearTimeout(shared);
  shared = window.setTimeout(() => (label.textContent = 'Share'), 1600);
});

chrome();
