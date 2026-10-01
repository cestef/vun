// The language server's worker waits on shared memory, which only a page isolated across origins
// has. Where the host sends no headers for that (GitHub Pages), a service worker (`public/coi.js`)
// puts them on the page's responses, and the page loads once more to get them. Where the page is
// isolated already, nothing is done.

const TRIED = 'vun.play.isolating';

// Resolves once the page is as isolated as it will be, or never when it is loading again.
export async function isolate(base: string): Promise<void> {
  if (crossOriginIsolated || !isSecureContext || !('serviceWorker' in navigator)) return;
  try {
    // Once a visit: a page the worker cannot isolate must not load forever.
    if (sessionStorage.getItem(TRIED) !== null) return;
    sessionStorage.setItem(TRIED, '1');
    await navigator.serviceWorker.register(`${base}coi.js`);
    await navigator.serviceWorker.ready;
  } catch {
    // Without a worker or a place to remember, the editor goes on without a server.
    return;
  }
  location.reload();
  await new Promise(() => {});
}
