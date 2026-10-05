// The text a build was made from, which the compiler's records count bytes of: the editor counts
// characters, so every place in a record is turned into one.

import type {At} from './protocol';
import {ROOT} from './wasi';

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export class Source {
  readonly text: string;
  private readonly bytes: Uint8Array;
  private readonly starts: number[];

  constructor(text: string) {
    this.text = text;
    this.bytes = encoder.encode(text);
    this.starts = [0];
    for (let i = 0; i < text.length; i++) if (text.charCodeAt(i) === 10) this.starts.push(i + 1);
  }

  // The character offset of the byte offset `at`.
  char(at: number): number {
    return decoder.decode(this.bytes.subarray(0, at)).length;
  }

  // Whether `at` is in the file the editor holds.
  holds(at: At): boolean {
    return at.file === ROOT || at.file.endsWith(`/${ROOT}`);
  }

  from(at: At): number {
    return this.char(at.bytes.start);
  }

  to(at: At): number {
    return this.char(at.bytes.end);
  }

  lines(): string[] {
    return this.text.split('\n');
  }

  // Where line `number` (from 1) starts, as a character offset.
  start(number: number): number {
    return this.starts[number - 1] ?? this.text.length;
  }

  // `edits`, as character ranges, applied to the text.
  patched(edits: {from: number; to: number; insert: string}[]): string {
    return [...edits].sort((a, b) => b.from - a.from).reduce((text, edit) => text.slice(0, edit.from) + edit.insert + text.slice(edit.to), this.text);
  }
}
