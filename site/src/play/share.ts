// Share links: the source compressed into the link's fragment, as short as it can be. Deflate
// is given a preset dictionary of Vun code (`share.vun`, programs of the corpus as they were),
// so the words and shapes every program repeats cost a few bits each. A link starts with the
// dictionary's version: the file is never edited, since links made with it read only with it; a
// new dictionary is a new file under the next version.

import {deflateSync, inflateSync, strFromU8, strToU8} from 'fflate';

import first from './share.vun?raw';

const DICTIONARIES: Record<string, Uint8Array> = {a: strToU8(first)};
const CURRENT = 'a';

const base64url = (bytes: Uint8Array) => btoa(String.fromCharCode(...bytes)).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
const bytes = (text: string) => Uint8Array.from(atob(text.replace(/-/g, '+').replace(/_/g, '/')), (c) => c.charCodeAt(0));

// The fragment holding `code`, without its `#`.
export function encode(code: string): string {
  return CURRENT + base64url(deflateSync(strToU8(code), {level: 9, mem: 12, dictionary: DICTIONARIES[CURRENT]}));
}

// The code the fragment `fragment` holds; `undefined` when it holds none that reads.
export function decode(fragment: string): string | undefined {
  const dictionary = DICTIONARIES[fragment.charAt(0)];
  if (dictionary === undefined) return undefined;
  try {
    return strFromU8(inflateSync(bytes(fragment.slice(1)), {dictionary}));
  } catch {
    return undefined;
  }
}
