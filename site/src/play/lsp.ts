// The page's end of the language server: a transport for the editor's client, over the server's
// worker. It needs shared memory, which only a page isolated across origins has.

import type {Transport} from '@codemirror/lsp-client';

import {Sender, shared} from './channel';
import type {Served, Serving, Toolchain} from './protocol';

const encoder = new TextEncoder();

// A transport to a server started now, and a promise of why it ended; `undefined` where memory
// cannot be shared.
export function serve(toolchain: Toolchain, code: string) {
  if (!crossOriginIsolated) return undefined;
  const memory = shared();
  const sender = new Sender(memory);
  const worker = new Worker(new URL('./server.ts', import.meta.url), {type: 'module'});
  let handlers: ((message: string) => void)[] = [];
  const ended = new Promise<string>((resolve) => {
    worker.onmessage = ({data}: MessageEvent<Served>) => {
      if (data.kind === 'message') for (const handler of handlers) handler(data.text);
      else if (data.kind === 'took') sender.pump();
      else resolve(data.why);
    };
    worker.onerror = (event) => resolve(event.message);
  });
  worker.postMessage({toolchain, code, input: memory} satisfies Serving);
  const transport: Transport = {
    send(message) {
      const body = encoder.encode(message);
      sender.send(encoder.encode(`Content-Length: ${body.length}\r\n\r\n`));
      sender.send(body);
    },
    subscribe: (handler) => void handlers.push(handler),
    unsubscribe: (handler) => void (handlers = handlers.filter((h) => h !== handler)),
  };
  return {transport, ended};
}
