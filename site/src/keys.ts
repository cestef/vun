// The keyboard, from one table: each binding is an action, the keys that do it, and the words that
// say it. The keys are handled for the whole page, the buttons that carry `data-action` show the
// keys on them and do the action when pressed, and the cheat sheet (F1, or ? outside the editor) is
// written from the same table, so the three never disagree.

import {element} from './play/dom';

type Group = 'Editor' | 'Panel' | 'Course' | 'Page';

interface Combo {
  // `KeyboardEvent.code`, so that a layout or the Option key of a Mac does not change the key.
  code: string;
  // Ctrl, or Cmd on a Mac.
  mod?: boolean;
  alt?: boolean;
  shift?: boolean;
}

export interface Binding {
  action: string;
  combo: Combo;
  group: Group;
  say: string;
}

export const BINDINGS: Binding[] = [
  {action: 'run', combo: {code: 'Enter', mod: true}, group: 'Editor', say: 'Run, or stop the run'},
  {action: 'next-problem', combo: {code: 'F8'}, group: 'Editor', say: 'Next problem'},
  {action: 'previous-problem', combo: {code: 'F8', shift: true}, group: 'Editor', say: 'Previous problem'},
  {action: 'focus-editor', combo: {code: 'Escape'}, group: 'Editor', say: 'Back to the editor'},
  {action: 'fold', combo: {code: 'KeyJ', alt: true}, group: 'Panel', say: 'Fold or unfold the panel'},
  {action: 'problems', combo: {code: 'Digit1', alt: true}, group: 'Panel', say: 'Show the problems'},
  {action: 'output', combo: {code: 'Digit2', alt: true}, group: 'Panel', say: 'Show the output'},
  {action: 'hint', combo: {code: 'KeyH', alt: true}, group: 'Course', say: 'More help, one step further'},
  {action: 'previous-lesson', combo: {code: 'ArrowLeft', alt: true}, group: 'Course', say: 'Previous lesson'},
  {action: 'next-lesson', combo: {code: 'ArrowRight', alt: true}, group: 'Course', say: 'Next lesson'},
  {action: 'map', combo: {code: 'KeyM', alt: true}, group: 'Course', say: 'The lessons'},
  {action: 'reset', combo: {code: 'KeyR', alt: true}, group: 'Course', say: 'Start the lesson over'},
  {action: 'playground', combo: {code: 'KeyP', alt: true}, group: 'Course', say: 'Open the code in the playground'},
  {action: 'share', combo: {code: 'KeyS', alt: true}, group: 'Page', say: 'Copy a link to the code'},
  {action: 'helix', combo: {code: 'KeyK', alt: true}, group: 'Page', say: 'Helix keys on or off'},
  {action: 'keys', combo: {code: 'F1'}, group: 'Page', say: 'This list'},
];

const MAC = /mac|iphone|ipad/i.test(navigator.platform);

const NAMES: Record<string, string> = {Enter: 'Enter', Escape: 'Esc', ArrowLeft: 'Left', ArrowRight: 'Right', ArrowUp: 'Up', ArrowDown: 'Down'};

// The keys of `combo` as the caps to press, in the order they are pressed.
export function caps({code, mod, alt, shift}: Combo): string[] {
  const key = NAMES[code] ?? code.replace(/^(Key|Digit)/, '');
  return [...(mod ? [MAC ? 'Cmd' : 'Ctrl'] : []), ...(alt ? [MAC ? 'Opt' : 'Alt'] : []), ...(shift ? ['Shift'] : []), key];
}

const matches = (event: KeyboardEvent, {code, mod, alt, shift}: Combo) =>
  event.code === code && (event.ctrlKey || event.metaKey) === (mod ?? false) && event.altKey === (alt ?? false) && event.shiftKey === (shift ?? false);

// Whether a key typed here is text: the cheat sheet's `?` must not steal a question mark.
const typing = (target: EventTarget | null) => target instanceof HTMLElement && target.closest('input, textarea, [contenteditable=true], .cm-content') !== null;

export function keycaps(combo: Combo): HTMLElement {
  return element('span', 'caps', ...caps(combo).map((cap) => element('kbd', '', cap)));
}

export class Keys {
  private readonly handlers = new Map<string, () => void>();
  private readonly sheet: HTMLDialogElement;

  constructor(private readonly page: ParentNode = document) {
    this.sheet = element('dialog', 'keys');
    document.body.append(this.sheet);
    this.sheet.addEventListener('click', (event) => event.target === this.sheet && this.sheet.close());
    this.on('keys', () => this.toggle());
    addEventListener('keydown', (event) => this.press(event), true);
  }

  // Does `run` for `action`: by the keys, and by each element that names it unless it is a link,
  // which goes where it points by itself.
  on(action: string, run: () => void) {
    this.handlers.set(action, run);
    for (const button of this.page.querySelectorAll<HTMLElement>(`[data-action="${action}"]`)) {
      if (!(button instanceof HTMLAnchorElement)) button.addEventListener('click', run);
      this.label(button, action);
    }
  }

  // Marks `button` with the keys of `action`, on it and in its tooltip.
  private label(button: HTMLElement, action: string) {
    const binding = BINDINGS.find((b) => b.action === action);
    if (binding === undefined) return;
    button.title = `${binding.say} (${caps(binding.combo).join(' ')})`;
    if (button.hasAttribute('data-keycaps')) button.append(keycaps(binding.combo));
  }

  private press(event: KeyboardEvent) {
    if (event.isComposing) return;
    if (event.key === '?' && !typing(event.target)) {
      event.preventDefault();
      return this.toggle();
    }
    const binding = BINDINGS.find((b) => this.handlers.has(b.action) && matches(event, b.combo));
    if (binding === undefined) return;
    // Escape is the editor's to take only from elsewhere on the page.
    if (binding.action === 'focus-editor' && (typing(event.target) || this.sheet.open)) return;
    event.preventDefault();
    event.stopPropagation();
    this.handlers.get(binding.action)?.();
  }

  private toggle() {
    if (this.sheet.open) return this.sheet.close();
    const groups = new Map<Group, Binding[]>();
    for (const binding of BINDINGS.filter((b) => this.handlers.has(b.action))) groups.set(binding.group, [...(groups.get(binding.group) ?? []), binding]);
    this.sheet.replaceChildren(
      element('h2', '', 'Keyboard'),
      ...[...groups].map(([group, bindings]) =>
        element('section', '', element('h3', '', group), ...bindings.map((b) => element('p', 'binding', element('span', '', b.say), keycaps(b.combo)))),
      ),
      element('p', 'note', 'Outside the editor, ? opens this list.'),
    );
    this.sheet.showModal();
  }
}
