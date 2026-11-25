// The brand's schemes as custom properties: the colors of `palette.toml` and of the code card,
// light by default, dark when the system asks and the page does not say light, and either when the
// page says so.

import {parse} from 'smol-toml';

import {source} from './source';

type Table = Record<string, Record<string, string>>;

function variables(scheme: string): string {
  const palette = (parse(source('assets/palette.toml')) as Table)[scheme];
  const code = (parse(source('assets/cards/code/colors.toml')) as Table)[scheme];
  const entries = [...Object.entries(palette).map(([name, color]) => `--${name}: ${color};`), ...Object.entries(code).map(([name, color]) => `--code-${name}: ${color};`)];
  return `color-scheme: ${scheme}; ${entries.join(' ')}`;
}

export function brandCss(): string {
  return [
    `:root { ${variables('light')} }`,
    `@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) { ${variables('dark')} } }`,
    `:root[data-theme="dark"] { ${variables('dark')} }`,
  ].join('\n');
}
