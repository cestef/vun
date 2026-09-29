// A session in a terminal window: each command typed after the prompt, then what it printed
// written line by line, in vun's styles as `pnpm record` captured them.

import {Layout, Rect, Txt} from '@motion-canvas/2d';
import {type ThreadGenerator, waitFor} from '@motion-canvas/core';

import type {CodeScheme} from '../brand/code';
import {STYLES} from '../brand/terminal';
import {ADVANCE, BOLD, FONT} from '../brand/type';
import {Window} from './Window';
import type {Line, Session} from './episode';

const PROMPT = '$ ';
// Spaces kept as printed, as a terminal shows them: text wraps and collapses ordinary spaces.
const SPACE = / /g;
const KEPT = '\u00a0';
const kept = (text: string) => text.replace(SPACE, KEPT);
// Seconds per character typed, before a command's output, between its lines.
const TYPING = 0.04;
const THINKING = 0.35;
const WRITING = 0.07;

const width = ({pieces}: Line) => pieces.reduce((sum, piece) => sum + piece.text.length, 0);
const command = (words: string[]) => words.join(' ');

export class Terminal {
  readonly window: Window;
  private readonly cursor: Rect;
  private readonly text;

  constructor(
    private readonly colors: CodeScheme,
    private readonly session: Session,
    font: number,
  ) {
    const {columns, rows} = Terminal.size(session);
    this.window = new Window(colors, session.directory, font, columns, rows);
    this.text = {fontFamily: FONT, fontSize: font, lineHeight: '150%', fill: colors.text} as const;
    this.cursor = (<Rect width={ADVANCE * font} height={font * 1.2} fill={colors.text} />) as Rect;
  }

  // The characters it shows across and down: each command and its output, then the prompt
  // left waiting.
  static size({steps}: Session): {columns: number; rows: number} {
    return {
      columns: Math.max(
        ...steps.map((step) => PROMPT.length + command(step.command).length),
        ...steps.flatMap((step) => step.output.map(width)),
      ),
      rows: steps.reduce((sum, step) => sum + 1 + step.output.length, 1),
    };
  }

  // Types each command and writes its output, leaving the prompt waiting.
  *play(): ThreadGenerator {
    for (const step of this.session.steps) {
      const typed = command(step.command);
      yield* this.prompt().text(kept(typed), typed.length * TYPING);
      yield* waitFor(THINKING);
      this.cursor.remove();
      for (const line of step.output) {
        this.write(line);
        yield* waitFor(WRITING);
      }
    }
    this.prompt();
  }

  private prompt(): Txt {
    const typed = (<Txt {...this.text} text="" />) as Txt;
    this.window.body.add(
      <Layout layout alignItems="center">
        <Txt {...this.text} fill={this.colors.comment} text={kept(PROMPT)} />
        {typed}
        {this.cursor}
      </Layout>,
    );
    return typed;
  }

  private write({pieces}: Line) {
    this.window.body.add(
      <Txt {...this.text} text={pieces.length === 0 ? KEPT : undefined}>
        {pieces.map(({style, text}) => {
          const look = style === undefined ? undefined : STYLES[style];
          return (
            <Txt fill={this.colors[look?.color ?? 'text']} fontWeight={look?.bold ? BOLD : undefined} text={kept(text)} />
          );
        })}
      </Txt>,
    );
  }
}
