// A service worker that gives the playground's page the headers that isolate it across origins,
// for hosts that cannot send them (GitHub Pages). The language server's worker waits on shared
// memory, which only an isolated page has. `src/play/isolate.ts` registers it.

self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (event) => event.waitUntil(self.clients.claim()));

self.addEventListener('fetch', (event) => {
  const request = event.request;
  // A request the browser can only answer from its cache, for another origin: not ours to answer.
  if (request.cache === 'only-if-cached' && request.mode !== 'same-origin') return;
  event.respondWith(
    fetch(request).then((response) => {
      // An opaque response has no headers to add to.
      if (response.status === 0) return response;
      const headers = new Headers(response.headers);
      headers.set('Cross-Origin-Opener-Policy', 'same-origin');
      headers.set('Cross-Origin-Embedder-Policy', 'require-corp');
      return new Response(response.body, {status: response.status, statusText: response.statusText, headers});
    }),
  );
});
