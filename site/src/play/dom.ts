// Making the page's elements, and the text of a compiler message as they show it.

import type {Message} from './protocol';

export function element<K extends keyof HTMLElementTagNameMap>(tag: K, className = '', ...children: (Node | string)[]) {
  const made = document.createElement(tag);
  made.className = className;
  made.append(...children);
  return made;
}

export function button(className: string, ...children: (Node | string)[]): HTMLButtonElement {
  const made = element('button', className, ...children);
  made.type = 'button';
  return made;
}

// A message's parts, code and types in `<code>`.
export function message(msg: Message): (Node | string)[] {
  return msg.parts.map((part) => (part.kind === 'text' ? part.value : element('code', `part ${part.kind}`, part.value)));
}
