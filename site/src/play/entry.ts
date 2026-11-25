// One diagnostic of a build, numbered as the report lists them, and the places of the editor it owns.

import type {Diagnostic, Label} from './protocol';
import type {Source} from './source';
import type {Role, Span} from './spans';

export class Entry {
  constructor(
    readonly diagnostic: Diagnostic,
    readonly index: number,
    readonly source: Source,
  ) {}

  // Where this problem is drawn in the editor.
  spans(): Span[] {
    const {diagnostic: d, source} = this;
    const spans: Span[] = [];
    const add = (label: Label | null, role: Role) => {
      if (label === null || !source.holds(label.at)) return;
      spans.push({problem: this.index, role, severity: d.severity, from: source.from(label.at), to: source.to(label.at), note: label.message?.text ?? null});
    };
    add(d.primary, 'primary');
    for (const label of d.labels) add(label, 'label');
    return spans;
  }
}

// The explanation page of the diagnostic `path` names in the library: its package's directory,
// then the path's names as directories, as the library lays packages out.
export function explanation(library: Record<string, string>, path: string): string | undefined {
  const names = path.split('.');
  return library[`${names[0]}/${names.join('/')}.md`];
}
