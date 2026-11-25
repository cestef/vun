// The color scheme the page is shown in: the system's unless the reader picked one, kept in this
// browser.

import {THEME_KEY as KEY} from './site';

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

const DARK = matchMedia('(prefers-color-scheme: dark)');

// The page always names its scheme, the system's when the reader picked none, so that styles
// written for `data-theme` (the guide's) and for the media query (the rest) agree.
function apply(theme: Theme) {
  document.documentElement.dataset.theme = theme === 'system' ? (DARK.matches ? 'dark' : 'light') : theme;
}

// Applies the stored theme, and makes each of `buttons` step through the themes.
export function themed(buttons: Iterable<HTMLButtonElement>) {
  let theme = stored();
  apply(theme);
  const shown = [...buttons];
  const show = () => {
    for (const button of shown) {
      button.dataset.theme = theme;
      button.title = `theme: ${theme}`;
    }
  };
  for (const button of shown) {
    button.addEventListener('click', () => {
      theme = THEMES[(THEMES.indexOf(theme) + 1) % THEMES.length];
      apply(theme);
      show();
      try {
        localStorage.setItem(KEY, theme);
      } catch {
        // Kept for this page only.
      }
    });
  }
  show();
  DARK.addEventListener('change', () => apply(theme));
}
