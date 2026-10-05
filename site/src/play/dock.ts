// The panel under the editor: tabs over views, one shown at a time, and a button that folds it to
// its tab bar so that the editor has the height. Which tab and whether it is folded are kept.

import {type IconName, icon} from '../icons';
import {button, element} from './dom';
import {keep, kept} from './store';

const FOLDED = 'vun.play.folded';

export class Tab {
  constructor(
    readonly id: string,
    readonly view: HTMLElement,
    private readonly handle: HTMLButtonElement,
    private readonly badge: HTMLElement,
  ) {}

  // The small mark beside the tab's name: a count, or nothing.
  mark(text: string, kind = '') {
    this.badge.textContent = text;
    this.badge.className = `badge ${kind}`;
    this.badge.hidden = text === '';
  }

  select(on: boolean) {
    this.handle.setAttribute('aria-selected', String(on));
    this.view.hidden = !on;
  }
}

export class Dock {
  private readonly tabs: Tab[] = [];
  private readonly bar: HTMLElement;
  private readonly views: HTMLElement;
  private readonly fold: HTMLButtonElement;

  constructor(readonly root: HTMLElement) {
    this.bar = root.querySelector('[data-tabs]') as HTMLElement;
    this.views = root.querySelector('[data-views]') as HTMLElement;
    this.fold = button('fold', icon('chevron-down'));
    this.fold.setAttribute('aria-label', 'Fold the panel');
    this.fold.addEventListener('click', () => this.toggle());
    this.bar.append(element('span', 'space'), this.fold);
    this.folded(kept(FOLDED) === 'true');
  }

  tab(id: string, label: string, view: HTMLElement, symbol: IconName): Tab {
    const handle = button('tab', icon(symbol), element('span', 'label', label));
    handle.setAttribute('role', 'tab');
    const badge = element('span', 'badge');
    badge.hidden = true;
    handle.append(badge);
    view.hidden = true;
    this.views.append(view);
    this.bar.insertBefore(handle, this.bar.querySelector('.space'));
    const tab = new Tab(id, view, handle, badge);
    handle.addEventListener('click', () => this.show(id));
    this.tabs.push(tab);
    if (this.tabs.length === 1) tab.select(true);
    return tab;
  }

  // Shows the tab `id`, unfolding the panel.
  show(id: string) {
    for (const tab of this.tabs) tab.select(tab.id === id);
    this.folded(false);
  }

  toggle() {
    this.folded(!this.root.classList.contains('folded'));
  }

  private folded(on: boolean) {
    this.root.classList.toggle('folded', on);
    this.fold.setAttribute('aria-expanded', String(!on));
    keep(FOLDED, String(on));
  }
}
