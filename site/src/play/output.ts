// What a program wrote, or a compiler that wrote no record, and how the run ended.

import {element} from './dom';

export class Output {
  private text: HTMLPreElement | undefined;

  constructor(private readonly view: HTMLElement) {}

  clear() {
    this.text = undefined;
    this.view.replaceChildren();
  }

  write(text: string) {
    this.text ??= this.view.appendChild(element('pre', 'program'));
    this.text.append(text);
    this.view.scrollTop = this.view.scrollHeight;
  }

  get empty(): boolean {
    return this.text === undefined;
  }
}
