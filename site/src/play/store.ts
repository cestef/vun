// What the page keeps between visits. Storage may be refused, as in a private window: then nothing
// outlives the page.

export function kept(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

export function keep(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Kept as long as the page.
  }
}
