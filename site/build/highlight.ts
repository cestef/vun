// Vun's code as the cards color it: the grammar the editors use, and a theme per scheme giving
// the roles of `cards/code/roles.toml` the colors of `cards/code/colors.toml`. The playground
// highlights with them as it is typed in.

import {parse} from 'smol-toml';

import {source} from './source';

type Scheme = Record<string, string>;

interface Role {
  scopes: string[];
  color: string;
  style: string;
}

// The colors a terminal's escape codes name, in the code card's: the compiler's own output is shown
// as it prints it. Red is the keyword's color, green the string's, blue and cyan the literal's.
function terminal(scheme: Scheme): Record<string, string> {
  const palette = {Black: scheme.text, Red: scheme.keyword, Green: scheme.string, Yellow: scheme.keyword, Blue: scheme.literal, Magenta: scheme.keyword, Cyan: scheme.literal, White: scheme.text};
  return Object.fromEntries(Object.entries(palette).flatMap(([name, color]) => [[`terminal.ansi${name}`, color], [`terminal.ansiBright${name}`, color]]));
}

export function highlighting() {
  const schemes = parse(source('assets/cards/code/colors.toml')) as unknown as Record<'light' | 'dark', Scheme>;
  const roles = (parse(source('assets/cards/code/roles.toml')) as unknown as {roles: Role[]}).roles;
  const themes = Object.entries(schemes).map(([name, scheme]) => ({
    name,
    fg: scheme.text,
    bg: scheme.background,
    colors: {'editor.background': scheme.background, 'editor.foreground': scheme.text, ...terminal(scheme)},
    settings: [
      {settings: {foreground: scheme.text}},
      ...roles.map(({scopes, color, style}) => ({scope: scopes, settings: {foreground: scheme[color], fontStyle: style}})),
    ],
  }));
  return {grammar: JSON.parse(source('editors/vscode/syntaxes/vun.tmLanguage.json')), themes};
}
