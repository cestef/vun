// A line diff between the code in the editor and a lesson's answer: the lines to change, and the
// lines to write, found by the longest run of lines the two share.

export type Change = {kind: 'same' | 'remove' | 'add'; text: string; line: number};

export function diff(from: string, to: string): Change[] {
  const a = from.split('\n');
  const b = to.split('\n');
  // How many lines of `a[i..]` and `b[j..]` the longest shared run holds.
  const shared = Array.from({length: a.length + 1}, () => new Array<number>(b.length + 1).fill(0));
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      shared[i][j] = a[i] === b[j] ? shared[i + 1][j + 1] + 1 : Math.max(shared[i + 1][j], shared[i][j + 1]);
    }
  }
  const changes: Change[] = [];
  let i = 0;
  let j = 0;
  while (i < a.length || j < b.length) {
    if (i < a.length && j < b.length && a[i] === b[j]) {
      changes.push({kind: 'same', text: a[i], line: i + 1});
      i++;
      j++;
    } else if (j < b.length && (i === a.length || shared[i][j + 1] >= shared[i + 1][j])) {
      changes.push({kind: 'add', text: b[j], line: i + 1});
      j++;
    } else {
      changes.push({kind: 'remove', text: a[i], line: i + 1});
      i++;
    }
  }
  return changes;
}

// The lines of `from` that differ from `to`: those removed, and those a line is to be added before.
export function differing(from: string, to: string): number[] {
  const lines = new Set<number>();
  for (const change of diff(from, to)) if (change.kind !== 'same') lines.add(change.line);
  return [...lines].sort((x, y) => x - y);
}
