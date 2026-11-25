// Vun's code in the colors of the cards, from the grammar the editors use: as HTML, and as the
// editor's decorations, recomputed as it is edited.

import {type HighlighterCore, createHighlighterCoreSync} from '@shikijs/core';
import {createJavaScriptRegexEngine} from '@shikijs/engine-javascript';
import {RangeSetBuilder} from '@codemirror/state';
import {Decoration, type DecorationSet, type EditorView, ViewPlugin, type ViewUpdate} from '@codemirror/view';
const highlight = __HIGHLIGHT__;

const THEMES = {light: 'light', dark: 'dark'};

let shiki: HighlighterCore | undefined;

function load(): HighlighterCore {
  shiki ??= createHighlighterCoreSync({engine: createJavaScriptRegexEngine(), langs: [highlight.grammar as never], themes: highlight.themes as never});
  return shiki;
}

// `code` as a `<pre>` of colored spans, each with its dark color as `--shiki-dark`.
export function colored(code: string): string {
  return load().codeToHtml(code, {lang: highlight.grammar.name, themes: THEMES, defaultColor: 'light'});
}

// The code blocks of `html` naming Vun, colored.
export function coloredBlocks(html: string): string {
  const template = document.createElement('template');
  template.innerHTML = html;
  for (const code of template.content.querySelectorAll('pre > code')) {
    if (!/\bvun\b/.test(code.className)) continue;
    code.parentElement?.replaceWith(template.ownerDocument.createRange().createContextualFragment(colored(code.textContent ?? '')));
  }
  return template.innerHTML;
}

const css = (style: Record<string, string>) => Object.entries(style).map(([name, value]) => `${name}:${value}`).join(';');

function decorations(view: EditorView): DecorationSet {
  const doc = view.state.doc;
  const {tokens} = load().codeToTokens(doc.toString(), {lang: highlight.grammar.name, themes: THEMES, defaultColor: 'light'});
  const builder = new RangeSetBuilder<Decoration>();
  for (const line of tokens) {
    for (const token of line) {
      if (token.htmlStyle === undefined || token.content.trim() === '') continue;
      builder.add(token.offset, token.offset + token.content.length, Decoration.mark({class: 'tok', attributes: {style: css(token.htmlStyle)}}));
    }
  }
  return builder.finish();
}

// The editor's colors, made again from the whole text after each change.
export const highlighting = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = decorations(view);
    }
    update(update: ViewUpdate) {
      if (update.docChanged) this.decorations = decorations(update.view);
    }
  },
  {decorations: (plugin) => plugin.decorations},
);
