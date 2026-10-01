// The editor: CodeMirror, colored as the cards color code, and served by `vun util lsp` when the
// page can run it: diagnostics as they are typed, hover, completion, signature help, rename,
// formatting and definitions.

import {autocompletion, closeBrackets, closeBracketsKeymap, completionKeymap} from '@codemirror/autocomplete';
import {defaultKeymap, history, historyKeymap, indentWithTab} from '@codemirror/commands';
import {bracketMatching, foldGutter, foldKeymap, indentOnInput, indentUnit} from '@codemirror/language';
import {lintKeymap} from '@codemirror/lint';
import {LSPClient, type Transport, formatKeymap, jumpToDefinitionKeymap, languageServerExtensions, renameKeymap} from '@codemirror/lsp-client';
import {Compartment, EditorState, type Extension, Prec, StateEffect} from '@codemirror/state';
import {highlightSelectionMatches, searchKeymap} from '@codemirror/search';
import {
  EditorView,
  ViewPlugin,
  type ViewUpdate,
  crosshairCursor,
  drawSelection,
  dropCursor,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
  rectangularSelection,
} from '@codemirror/view';
import {helix} from 'codemirror-helix';

import {applyFix, caretFixes, published} from './fixes';
import {coloredBlocks, highlighting} from './highlight';

// The document as the server names it.
export const URI = 'file:///main.vun';
const INDENT = '    ';
// How long a request may take: the first check reads the whole library.
const TIMEOUT = 20_000;

// The caret blinks through the page's own animation (`play.css`), as Helix stops CodeMirror's:
// two alike, swapped whenever the caret moves or the text changes, so it starts again shown.
const blinking = ViewPlugin.fromClass(
  class {
    constructor(private readonly view: EditorView) {
      view.dom.classList.add('caret-a');
    }
    update(update: ViewUpdate) {
      if (!update.selectionSet && !update.docChanged && !update.focusChanged) return;
      this.view.dom.classList.toggle('caret-a');
      this.view.dom.classList.toggle('caret-b');
    }
  },
);

// What every editor has, as CodeMirror's basic setup gives it, but its colors (the grammar's are
// used) and its marks of the text matching the selection, which a mode lays on (`Editor::helix`).
const setup = [
  lineNumbers(),
  highlightActiveLineGutter(),
  highlightSpecialChars(),
  history(),
  foldGutter(),
  drawSelection({cursorBlinkRate: 0}),
  dropCursor(),
  EditorState.allowMultipleSelections.of(true),
  indentOnInput(),
  bracketMatching(),
  closeBrackets(),
  autocompletion(),
  rectangularSelection(),
  crosshairCursor(),
  highlightActiveLine(),
  keymap.of([...closeBracketsKeymap, ...defaultKeymap, ...searchKeymap, ...historyKeymap, ...foldKeymap, ...completionKeymap, ...lintKeymap]),
];

// The usual keys mark the text matching the selection; Helix's do not, as its caret is a
// selection of one character, which would mark every alike character as it moves.
const PLAIN = highlightSelectionMatches();
const HELIX = helix({config: {'editor.cursor-shape.insert': 'bar'}});

const theme = EditorView.theme({
  '&': {height: '100%', fontSize: '13.5px', backgroundColor: 'transparent', color: 'var(--code-text)'},
  '&.cm-focused': {outline: 'none'},
  '.cm-scroller': {fontFamily: 'inherit', lineHeight: '1.6'},
  '.cm-content': {padding: '14px 0', caretColor: 'var(--ink)'},
  '.cm-gutters': {backgroundColor: 'transparent', border: 'none', color: 'var(--code-comment)', userSelect: 'none'},
  '.cm-activeLine, .cm-activeLineGutter': {backgroundColor: 'color-mix(in srgb, var(--echo) 55%, transparent)'},
  '&.cm-focused .cm-selectionBackground, .cm-selectionBackground': {backgroundColor: 'color-mix(in srgb, var(--accent) 28%, transparent) !important'},
  '.cm-matchingBracket': {backgroundColor: 'var(--echo)', outline: '1px solid var(--code-border)'},
  '.cm-tooltip': {border: '1px solid var(--code-border)', borderRadius: '10px', backgroundColor: 'var(--code-bar)', color: 'var(--code-text)', overflow: 'hidden'},
  '.cm-tooltip-autocomplete > ul > li[aria-selected]': {backgroundColor: 'var(--accent)', color: 'var(--background)'},
  '.cm-panels': {backgroundColor: 'var(--code-bar)', color: 'var(--code-text)', borderColor: 'var(--code-border)'},
});

export class Editor {
  readonly view: EditorView;
  private client: LSPClient | undefined;
  // Helix's keys and modes, or none.
  private readonly keys = new Compartment();

  // `changed` is called with the text after each edit, `run` on Ctrl or Cmd and Enter.
  constructor(parent: HTMLElement, code: string, changed: (code: string) => void, run: () => void) {
    const extensions: Extension[] = [
      Prec.highest(keymap.of([{key: 'Mod-Enter', run: () => (run(), true)}])),
      this.keys.of(PLAIN),
      setup,
      blinking,
      keymap.of([indentWithTab]),
      indentUnit.of(INDENT),
      EditorState.tabSize.of(INDENT.length),
      EditorState.languageData.of(() => [{commentTokens: {line: '//'}}]),
      highlighting,
      theme,
      EditorView.lineWrapping,
      EditorView.updateListener.of((update) => update.docChanged && changed(update.state.doc.toString())),
    ];
    this.view = new EditorView({parent, state: EditorState.create({doc: code, extensions})});
  }

  get code(): string {
    return this.view.state.doc.toString();
  }

  set code(code: string) {
    this.view.dispatch({changes: {from: 0, to: this.view.state.doc.length, insert: code}});
  }

  // Serves the editor from `transport`'s server from now on.
  connect(transport: Transport) {
    this.client = new LSPClient({
      rootUri: 'file:///',
      timeout: TIMEOUT,
      extensions: languageServerExtensions(),
      // The server is this page's own; its Markdown is only colored.
      sanitizeHTML: coloredBlocks,
      notificationHandlers: {'textDocument/publishDiagnostics': published},
    }).connect(transport);
    const keys = keymap.of([{key: 'Mod-.', run: applyFix}, ...formatKeymap, ...renameKeymap, ...jumpToDefinitionKeymap]);
    this.view.dispatch({effects: StateEffect.appendConfig.of([this.client.plugin(URI, 'vun'), keys, caretFixes])});
  }

  // Edits with Helix's keys and modes, or with the usual ones: a block caret outside insert mode.
  helix(on: boolean) {
    this.view.dispatch({effects: this.keys.reconfigure(on ? HELIX : PLAIN)});
  }

  // Selects the text from `from` to `to` and shows it.
  select(from: number, to: number) {
    this.view.dispatch({selection: {anchor: from, head: to}, scrollIntoView: true});
    this.view.focus();
  }

  // Replaces each range of the text with its text, all at once.
  change(edits: {from: number; to: number; insert: string}[]) {
    this.view.dispatch({changes: edits});
    this.view.focus();
  }
}
