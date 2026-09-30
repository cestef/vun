// The server's diagnostics with their fixes: the client this editor uses shows diagnostics
// without fixes, so publishing them is handled here. They are shown at once, as the client
// would, then the fixes of the whole document are asked once and kept by diagnostic. The fixes
// of a diagnostic the caret is in are shown under it, and stay while the caret does, where a
// hover would close as the pointer leaves the text for them; `Mod-.` applies the preferred one.

import {type Action, type Diagnostic, forEachDiagnostic, setDiagnostics, setDiagnosticsEffect} from '@codemirror/lint';
import {type LSPClient, LSPPlugin} from '@codemirror/lsp-client';
import {type EditorState, StateField, type Text} from '@codemirror/state';
import {type Command, type EditorView, type Tooltip, showTooltip} from '@codemirror/view';

interface Position {
  line: number;
  character: number;
}

interface Range {
  start: Position;
  end: Position;
}

interface Published {
  range: Range;
  severity?: number;
  message: string;
}

interface CodeAction {
  title: string;
  isPreferred?: boolean;
  diagnostics?: Published[];
  edit?: {documentChanges?: {textDocument: {uri: string; version: number | null}; edits: {range: Range; newText: string}[]}[]};
}

const SEVERITIES = ['error', 'error', 'warning', 'info', 'hint'] as const;

const same = (a: Published, b: Published) => a.message === b.message && JSON.stringify(a.range) === JSON.stringify(b.range);

// The fixes of each diagnostic, the preferred one first.
const fixesOf = new WeakMap<Diagnostic, Action[]>();

// `action`'s edits of this document as an action, applied while the text the server saw is still
// the one synchronized, through the changes made since.
function action(plugin: LSPPlugin, synced: Text, fix: CodeAction): Action {
  const edits = fix.edit?.documentChanges?.filter((change) => change.textDocument.uri === plugin.uri).flatMap((change) => change.edits) ?? [];
  return {
    name: fix.title,
    apply(view) {
      if (plugin.syncedDoc !== synced) return;
      const at = (position: Position) => plugin.unsyncedChanges.mapPos(plugin.fromPosition(position, synced));
      view.dispatch({changes: edits.map((edit) => ({from: at(edit.range.start), to: at(edit.range.end), insert: edit.newText}))});
    },
  };
}

function show(view: EditorView, plugin: LSPPlugin, synced: Text, published: Published[], fixes: CodeAction[]) {
  const diagnostics = published.map((item) => {
    const mine = fixes.filter((fix) => fix.diagnostics?.some((d) => same(d, item)));
    mine.sort((a, b) => Number(b.isPreferred ?? false) - Number(a.isPreferred ?? false));
    const diagnostic: Diagnostic = {
      from: plugin.unsyncedChanges.mapPos(plugin.fromPosition(item.range.start, synced)),
      to: plugin.unsyncedChanges.mapPos(plugin.fromPosition(item.range.end, synced)),
      severity: SEVERITIES[item.severity ?? 1],
      message: item.message,
    };
    if (mine.length > 0) fixesOf.set(diagnostic, mine.map((fix) => action(plugin, synced, fix)));
    return diagnostic;
  });
  view.dispatch(setDiagnostics(view.state, diagnostics));
}

// Handles `textDocument/publishDiagnostics` for the documents this client holds.
export function published(client: LSPClient, params: {uri: string; version?: number; diagnostics: Published[]}): boolean {
  const file = client.workspace.getFile(params.uri);
  if (!file || (params.version != null && params.version !== file.version)) return false;
  const view = file.getView();
  const plugin = view && LSPPlugin.get(view);
  if (!view || !plugin) return false;
  const synced = plugin.syncedDoc;
  show(view, plugin, synced, params.diagnostics, []);
  if (params.diagnostics.length === 0) return true;
  const whole = {start: plugin.toPosition(0, synced), end: plugin.toPosition(synced.length, synced)};
  const asked = {textDocument: {uri: params.uri}, range: whole, context: {diagnostics: params.diagnostics, only: ['quickfix']}};
  client.request<typeof asked, CodeAction[] | null>('textDocument/codeAction', asked).then(
    (fixes) => plugin.syncedDoc === synced && show(view, plugin, synced, params.diagnostics, fixes ?? []),
    () => undefined,
  );
  return true;
}

// The first diagnostic at the caret with fixes, where it starts, and its fixes.
function atCaret(state: EditorState): {from: number; fixes: Action[]} | undefined {
  const at = state.selection.main.head;
  let found: {from: number; fixes: Action[]} | undefined;
  forEachDiagnostic(state, (diagnostic, from, to) => {
    const fixes = fixesOf.get(diagnostic);
    if (found === undefined && fixes !== undefined && from <= at && at <= to) found = {from, fixes};
  });
  return found;
}

// Applies the preferred fix of the diagnostic at the caret.
export const applyFix: Command = (view) => {
  const found = atCaret(view.state);
  found?.fixes[0].apply(view, 0, 0);
  return found !== undefined;
};

function panel(view: EditorView, fixes: Action[]): HTMLElement {
  const dom = document.createElement('div');
  dom.className = 'cm-fixes';
  for (const [i, fix] of fixes.entries()) {
    const button = document.createElement('button');
    button.type = 'button';
    button.textContent = fix.name;
    if (i === 0) button.append(Object.assign(document.createElement('kbd'), {textContent: 'ctrl .'}));
    // The editor keeps its focus and caret while the fix is clicked.
    button.addEventListener('mousedown', (event) => event.preventDefault());
    button.addEventListener('click', () => fix.apply(view, 0, 0));
    dom.append(button);
  }
  return dom;
}

function tooltip(state: EditorState): Tooltip | null {
  const found = atCaret(state);
  return found ? {pos: found.from, above: false, create: (view) => ({dom: panel(view, found.fixes)})} : null;
}

// The fixes of the diagnostic at the caret, under it, from the caret's moving to its leaving.
export const caretFixes = StateField.define<Tooltip | null>({
  create: tooltip,
  update(value, transaction) {
    const moved = transaction.docChanged || transaction.selection || transaction.effects.some((effect) => effect.is(setDiagnosticsEffect));
    return moved ? tooltip(transaction.state) : value;
  },
  provide: (field) => showTooltip.from(field),
});
