// The language server's worker: `vun util lsp` over a standard input the page writes into shared
// memory, its messages read out of its standard output as they are framed.

import {ConsoleStdout, PreopenDirectory} from '@bjorn3/browser_wasi_shim';

import {Receiver} from './channel';
import type {Served, Serving} from './protocol';
import {ENV, command, executable, filesystem} from './wasi';

const tell = (served: Served) => postMessage(served);

const HEADERS_END = new TextEncoder().encode('\r\n\r\n');
const LENGTH = /content-length:\s*(\d+)/i;

// The server's output, cut into messages at the lengths their headers give.
function messages(): ConsoleStdout {
  const decoder = new TextDecoder();
  let pending = new Uint8Array();
  const end = () => pending.findIndex((_, i) => HEADERS_END.every((b, j) => pending[i + j] === b));
  return new ConsoleStdout((bytes) => {
    const joined = new Uint8Array(pending.length + bytes.length);
    joined.set(pending);
    joined.set(bytes, pending.length);
    pending = joined;
    for (let at = end(); at >= 0; at = end()) {
      const length = Number(LENGTH.exec(decoder.decode(pending.subarray(0, at)))?.[1] ?? 0);
      const body = at + HEADERS_END.length;
      if (pending.length < body + length) return;
      tell({kind: 'message', text: decoder.decode(pending.subarray(body, body + length))});
      pending = pending.slice(body + length);
    }
  });
}

onmessage = async ({data}: MessageEvent<Serving>) => {
  const dir = new PreopenDirectory('/', filesystem(data.toolchain, data.code));
  const input = new Receiver(data.input, () => tell({kind: 'took'}));
  const args = [executable(data.toolchain), 'util', 'lsp'];
  const ended = await command(data.toolchain.compiler, args, ENV, dir, messages(), input);
  tell({kind: 'ended', why: 'code' in ended ? `it exited with ${ended.code}` : ended.trap});
};
