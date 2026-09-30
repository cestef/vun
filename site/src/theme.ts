// The color scheme the page is shown in: the system's unless the reader picked one, kept in this
// browser.

import {make} from './svg';

const KEY = 'vun.theme';
const THEMES = ['system', 'light', 'dark'] as const;
type Theme = (typeof THEMES)[number];

function stored(): Theme {
  try {
    const theme = localStorage.getItem(KEY);
    return THEMES.find((known) => known === theme) ?? 'system';
  } catch {
    return 'system';
  }
}

function apply(theme: Theme) {
  if (theme === 'system') {
    delete document.documentElement.dataset.theme;
  } else {
    document.documentElement.dataset.theme = theme;
  }
}

// The button's icon, drawn round as the mark is: a ring, and inside it a fill sweeping in from
// the right as far as the theme is dark (none for light, half for the system's, all for dark),
// its extent set by the stylesheet from the button's `data-theme`.
function icon(): SVGSVGElement {
  const inside = make('clipPath', {id: 'theme-inside'}, make('circle', {cx: 12, cy: 12, r: 5.6}));
  const fill = make('g', {'clip-path': 'url(#theme-inside)'}, make('rect', {class: 'fill', x: 6, y: 6, width: 12, height: 12}));
  return make('svg', {viewBox: '0 0 24 24', 'aria-hidden': 'true'}, make('defs', {}, inside), make('circle', {class: 'ring', cx: 12, cy: 12, r: 8.4}), fill);
}

// Applies the stored theme, and makes `button` step through the themes.
export function themed(button: HTMLButtonElement | null) {
  let theme = stored();
  apply(theme);
  if (button === null) return;
  button.append(icon());
  button.dataset.theme = theme;
  button.addEventListener('click', () => {
    theme = THEMES[(THEMES.indexOf(theme) + 1) % THEMES.length];
    apply(theme);
    button.dataset.theme = theme;
    button.title = `theme: ${theme}`;
    try {
      localStorage.setItem(KEY, theme);
    } catch {
      // Kept for this page only.
    }
  });
  button.title = `theme: ${theme}`;
}
