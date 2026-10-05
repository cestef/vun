// The compiler, its library and the memos kept from earlier visits, loaded once for a page; each
// worker is given them.

import build from 'virtual:toolchain';

import * as cache from './cache';
import {isolate} from './isolate';
import type {Toolchain} from './protocol';

export function loadToolchain(base: string): Promise<Toolchain> {
  return (async () => {
    // Before the downloads, which a page about to load again would make twice.
    await isolate(base);
    const executable = new Uint8Array(await (await fetch(`${base}play/vun.wasm`)).arrayBuffer());
    const [compiler, library, saved] = await Promise.all([
      WebAssembly.compile(executable),
      fetch(`${base}play/library.json`).then((r) => r.json() as Promise<Record<string, string>>),
      cache.load(build),
    ]);
    return {id: build, compiler, executable, library, saved};
  })();
}
