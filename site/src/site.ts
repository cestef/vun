// Where the site is served from: its pages sit under `base`, which a project's GitHub Pages
// address has and a custom domain does not.

const BASE = import.meta.env.BASE_URL.replace(/\/$/, '');

// The address of `path`, a path from the site's root without the leading slash.
export const href = (path = '') => `${BASE}/${path}`;

// The pages of the site, in the order the bar lists them.
export const SECTIONS = [
  {name: 'docs', path: 'docs/', section: 'docs/'},
  {name: 'learn', path: 'docs/learn/', section: 'docs/learn/'},
  {name: 'play', path: 'play/', section: 'play/'},
] as const;

export const VERSION = __VERSION__;

export const REPOSITORY = 'https://github.com/cestef/vun';

// The one theme key the pages keep: see `theme.ts`.
export const THEME_KEY = 'vun.theme';
