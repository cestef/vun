// The keyboard, from one table: each binding is an action, the keys that do it, and the words that
// say it. The keys are handled for the whole page, the buttons of `Action` show the keys on them
// and do the action when pressed, and the cheat sheet (F1, or ? outside the editor) is written from
// the same table, so the three never disagree.

import {Cell} from './cell';

export type Group = 'Editor' | 'Panel' | 'Course' | 'Page';

export interface Combo {
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
  {action: 'reset', combo: {code: 'KeyR', alt: true}, group: 'Course', say: 'Start the lesson over'},
  {action: 'playground', combo: {code: 'KeyP', alt: true}, group: 'Course', say: 'Open the code in the playground'},
  {action: 'share', combo: {code: 'KeyS', alt: true}, group: 'Page', say: 'Copy a link to the code'},
  {action: 'helix', combo: {code: 'KeyK', alt: true}, group: 'Page', say: 'Helix keys on or off'},
  {action: 'keys', combo: {code: 'F1'}, group: 'Page', say: 'This list'},
];

const MAC = typeof navigator !== 'undefined' && /mac|iphone|ipad/i.test(navigator.platform);

const NAMES: Record<string, string> = {Enter: 'Enter', Escape: 'Esc', ArrowLeft: 'Left', ArrowRight: 'Right', ArrowUp: 'Up', ArrowDown: 'Down'};

// The keys of `combo` as the caps to press, in the order they are pressed.
export function caps({code, mod, alt, shift}: Combo): string[] {
  const key = NAMES[code] ?? code.replace(/^(Key|Digit)/, '');
  return [...(mod ? [MAC ? 'Cmd' : 'Ctrl'] : []), ...(alt ? [MAC ? 'Opt' : 'Alt'] : []), ...(shift ? ['Shift'] : []), key];
}

export const bindingOf = (action: string) => BINDINGS.find((binding) => binding.action === action);

const matches = (event: KeyboardEvent, {code, mod, alt, shift}: Combo) =>
  event.code === code && (event.ctrlKey || event.metaKey) === (mod ?? false) && event.altKey === (alt ?? false) && event.shiftKey === (shift ?? false);

// Whether a key typed here is text: the cheat sheet's `?` must not steal a question mark.
const typing = (target: EventTarget | null) => target instanceof HTMLElement && target.closest('input, textarea, [contenteditable=true], .cm-content') !== null;

// The handlers of the page's actions, and whether the cheat sheet is open.
export class Keys {
  private readonly handlers = new Map<string, () => void>();
  private readonly shown = new Cell(false);

  constructor() {
    addEventListener('keydown', (event) => this.press(event), true);
    this.on('keys', () => this.toggle());
  }

  get open(): boolean {
    return this.shown.value;
  }

  set open(open: boolean) {
    this.shown.value = open;
  }

  // The bindings of the actions this page handles.
  get listed(): Binding[] {
    return BINDINGS.filter((binding) => this.handlers.has(binding.action));
  }

  // Does `run` for `action`, replacing what did it before.
  on(action: string, run: () => void) {
    this.handlers.set(action, run);
  }

  fire(action: string) {
    this.handlers.get(action)?.();
  }

  toggle() {
    this.open = !this.open;
  }

  private press(event: KeyboardEvent) {
    if (event.isComposing) return;
    if (event.key === '?' && !typing(event.target)) {
      event.preventDefault();
      return this.toggle();
    }
    const binding = BINDINGS.find((b) => this.handlers.has(b.action) && matches(event, b.combo));
    if (binding === undefined) return;
    if (binding.action === 'focus-editor' && (typing(event.target) || this.open)) return;
    event.preventDefault();
    event.stopPropagation();
    this.fire(binding.action);
  }
}
