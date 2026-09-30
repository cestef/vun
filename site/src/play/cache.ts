// The memos the compiler saved, kept in this browser between visits, for the build of the
// compiler that saved them only: another build's are removed when this one keeps its own.

type Files = [string, Uint8Array][];

const NAME = 'vun-play';
const STORE = 'memos';

function open(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(NAME, 1);
    request.onupgradeneeded = () => request.result.createObjectStore(STORE);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

function done(request: IDBRequest | IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    if (request instanceof IDBTransaction) request.oncomplete = () => resolve();
    else request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error);
  });
}

// The memos kept for the build `id`; none where storage is refused, as in a private window.
export async function load(id: string): Promise<Files> {
  try {
    const request = (await open()).transaction(STORE).objectStore(STORE).get(id);
    await done(request);
    return (request.result as Files | undefined) ?? [];
  } catch {
    return [];
  }
}

export async function keep(id: string, files: Files) {
  try {
    const transaction = (await open()).transaction(STORE, 'readwrite');
    const store = transaction.objectStore(STORE);
    store.clear();
    store.put(files, id);
    await done(transaction);
  } catch {
    // Kept for this visit only, in the workers.
  }
}
