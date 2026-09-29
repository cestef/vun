// A program in a window, highlighted as the code card highlights it: its lines written in one by
// one, ranges of them picked out, or its text changed into another.

import {Code, lines} from '@motion-canvas/2d';
import {DEFAULT, type ThreadGenerator} from '@motion-canvas/core';

import {type CodeScheme, VunHighlighter} from '../brand/code';
import {FONT} from '../brand/type';
import {LEADING, Window} from './Window';
import {beat} from './mode';

// Seconds per line written, to pick out a range, to hold it, to change the text.
const WRITING = 0.12;
const PICKING = 0.5;
const FOCUS = 1.8;
const CHANGING = 1.2;

export class Program {
  readonly window: Window;
  readonly code: Code;

  // Sized for the longest and tallest of `texts`, the first of them written by `write`.
  constructor(
    colors: CodeScheme,
    scheme: string,
    file: string,
    private readonly texts: string[],
    font: number,
  ) {
    const {columns, rows} = Program.size(texts);
    this.window = new Window(colors, file, font, columns, rows);
    this.code = (
      <Code highlighter={new VunHighlighter(scheme)} fontFamily={FONT} fontSize={font} lineHeight={LEADING * font} code="" />
    ) as Code;
    this.window.body.add(this.code);
  }

  static size(texts: string[]): {columns: number; rows: number} {
    const rows = texts.map((text) => text.split('\n'));
    return {
      columns: Math.max(...rows.flat().map((row) => row.length)),
      rows: Math.max(...rows.map((text) => text.length)),
    };
  }

  *write(): ThreadGenerator {
    for (const row of this.texts[0].split('\n')) {
      yield* this.code.code.append(`${row}\n`, WRITING);
    }
  }

  // Picks out each range of `focus` in turn, lines counted from 1, then shows all again.
  *focus(focus: [number, number][]): ThreadGenerator {
    for (const [i, [from, to]] of focus.entries()) {
      yield* this.code.selection(lines(from - 1, to - 1), PICKING);
      yield* beat(`focus ${i + 1}`, FOCUS);
    }
    yield* this.code.selection(DEFAULT, PICKING);
  }

  // Changes the text into `texts[index]`, what stays moving into place.
  *change(index: number): ThreadGenerator {
    yield* this.code.code(`${this.texts[index]}\n`, CHANGING);
  }
}
