// Vun's code as the cards color it: the grammar the editors use, the roles of
// `cards/code/roles.toml` in the colors of `cards/code/colors.toml`, as a highlighter of the
// `Code` node.

import type {CodeHighlighter, HighlightResult} from '@motion-canvas/2d';
import {type HighlighterCore, type ThemedToken, createHighlighterCoreSync} from '@shikijs/core';
import {createJavaScriptRegexEngine} from '@shikijs/engine-javascript';
import {parse} from 'smol-toml';

import colors from '../../../cards/code/colors.toml?raw';
import roles from '../../../cards/code/roles.toml?raw';
import grammar from '../../../../editors/vscode/syntaxes/vun.tmLanguage.json';
import {entry} from './palette';

export interface CodeScheme {
  background: string;
  border: string;
  bar: string;
  text: string;
  comment: string;
  keyword: string;
  name: string;
  string: string;
  literal: string;
  punct: string;
}

interface Role {
  scopes: string[];
  color: keyof CodeScheme;
  style: string;
}

export const CODE_SCHEMES = parse(colors) as unknown as Partial<Record<string, CodeScheme>>;
const ROLES = (parse(roles) as unknown as {roles: Role[]}).roles;

export function codeScheme(name: string): CodeScheme {
  return entry(CODE_SCHEMES, name, 'code scheme');
}

// The token covering each character of a text.
type Colored = ThemedToken[];

export class VunHighlighter implements CodeHighlighter<Colored> {
  private static shiki: HighlighterCore | undefined;

  constructor(private readonly scheme: string) {}

  private static load(): HighlighterCore {
    VunHighlighter.shiki ??= createHighlighterCoreSync({
      engine: createJavaScriptRegexEngine(),
      langs: [grammar as never],
      themes: Object.entries(CODE_SCHEMES).map(([name, scheme]) => ({
        name,
        fg: scheme!.text,
        bg: scheme!.background,
        settings: [
          {settings: {foreground: scheme!.text}},
          ...ROLES.map(({scopes, color, style}) => ({scope: scopes, settings: {foreground: scheme![color], fontStyle: style}})),
        ],
      })),
    });
    return VunHighlighter.shiki;
  }

  initialize(): boolean {
    codeScheme(this.scheme);
    VunHighlighter.load();
    return true;
  }

  // The tokens of `code`, the line breaks as tokens of their own, so they spell it whole.
  private tokens(code: string): ThemedToken[] {
    const lines = VunHighlighter.load().codeToTokensBase(code, {lang: grammar.name, theme: this.scheme});
    const tokens = lines.flatMap((line, i) => (i === 0 ? line : [{content: '\n', offset: 0}, ...line]));
    let offset = 0;
    for (const token of tokens) {
      token.offset = offset;
      offset += token.content.length;
    }
    return tokens;
  }

  prepare(code: string): Colored {
    return this.tokens(code).flatMap((token) => Array<ThemedToken>(token.content.length).fill(token));
  }

  highlight(index: number, at: Colored): HighlightResult {
    const token = at.at(index);
    if (token === undefined) {
      return {color: null, skipAhead: 1};
    }
    return {color: token.color ?? null, skipAhead: token.offset + token.content.length - index};
  }

  tokenize(code: string): string[] {
    return this.tokens(code).map((token) => token.content);
  }
}
