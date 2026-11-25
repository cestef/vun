// The guide's groups, in order: the sidebar lists them and the guide's first page indexes them. A
// group names its pages by id, or a directory whose pages it lists in their own order.

export interface Group {
  label: string;
  pages?: string[];
  directory?: string;
}

export const GUIDE: Group[] = [
  {label: 'Start', pages: ['docs/install', 'docs/first-program']},
  {label: 'Learn', directory: 'docs/learn'},
  {label: 'Use', pages: ['docs/commands', 'docs/diagnostics', 'docs/testing']},
  {label: 'Build', pages: ['docs/packages', 'docs/targets']},
  {label: 'Maintain', pages: ['docs/toolchains']},
];
