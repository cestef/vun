// What the page and the workers tell each other, and the records `vun --format json` writes.

// The compiler as the workers run it: its build, its module and the bytes it was compiled from,
// the library by path, and the memos an earlier run saved, by path.
export interface Toolchain {
  id: string;
  compiler: WebAssembly.Module;
  executable: Uint8Array;
  library: Record<string, string>;
  saved: [string, Uint8Array][];
}

// What the build worker is asked: to lay out the toolchain, once, then to build and run sources.
export type Asked = {kind: 'setup'; toolchain: Toolchain} | {kind: 'run'; code: string};

// Which of the two programs a run starts wrote something: the compiler, or the program it built.
export type Stage = 'build' | 'run';

export type Told =
  | {kind: 'record'; record: Reported}
  // Output that is not a record: what the program writes, or the compiler's when it is not JSON.
  | {kind: 'output'; stage: Stage; text: string}
  | {kind: 'started'; stage: Stage}
  | {kind: 'ended'; stage: Stage; code: number}
  // A trap, such as a panic of the compiler or an out-of-bounds access of the program.
  | {kind: 'crashed'; stage: Stage; why: string}
  // The memos the compiler saved once it ended, by path.
  | {kind: 'saved'; files: [string, Uint8Array][]};

// What the language server's worker is given, and what it says.
export interface Serving {
  toolchain: Toolchain;
  code: string;
  input: SharedArrayBuffer;
}

export type Served = {kind: 'message'; text: string} | {kind: 'took'} | {kind: 'ended'; why: string};

// A piece of a message: text, or code, a type or another kind a renderer may set apart.
export interface Part {
  kind: string;
  value: string;
}

export interface Message {
  text: string;
  parts: Part[];
}

export interface At {
  file: string;
  start: {line: number; column: number};
  end: {line: number; column: number};
  bytes: {start: number; end: number};
}

export interface Label {
  at: At;
  message: Message | null;
}

export interface Fix {
  message: Message;
  safety: string;
  preferred: boolean;
  edits: {at: At; text: string}[];
}

export interface Child {
  kind: string;
  message: Message;
  labels: Label[];
  fixes: Fix[];
}

export interface Diagnostic {
  type: 'diagnostic';
  id: string;
  path: string;
  severity: string;
  message: Message;
  primary: Label | null;
  labels: Label[];
  children: Child[];
  fixes: Fix[];
}

export type Reported =
  | Diagnostic
  | {type: 'started'; step: string; subject: string}
  | {type: 'finished'; subject: string; success: boolean; milliseconds: number; errors: number; warnings: number};
