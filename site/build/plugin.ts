// What the pages read from the repository, made at build time:
//
// - `virtual:brand.css`: the schemes of `palette.toml` and `cards/code/colors.toml` as custom
//   properties, light by default, dark when the system asks and the page does not say light.
// - `virtual:highlight`: the grammar and themes the playground highlights with.
// - `virtual:lessons`: the guide of the playground, a tour of the language in `site/lessons`.
// - `virtual:toolchain`: the build of the compiler served, by the hash of its bytes.
// - `play/vun.wasm` and `play/library.json`: the compiler built for WASI and the library it
//   compiles against, which the playground runs in the browser. The compiler is built with
//   `cargo build -p vun-cli --target wasm32-wasip1 --profile dist`.

import {createHash} from 'node:crypto';

import {parse} from 'smol-toml';
import type {Plugin} from 'vite';

import {highlighting} from './highlight';
import {lessons} from './lessons';
import {REPOSITORY_DIR, bytes, files, source} from './source';

type Table = Record<string, Record<string, string>>;

function brand(): string {
  const variables = (scheme: string) => {
    const palette = (parse(source('assets/palette.toml')) as Table)[scheme];
    const code = (parse(source('assets/cards/code/colors.toml')) as Table)[scheme];
    const entries = [
      ...Object.entries(palette).map(([name, color]) => `--${name}: ${color};`),
      ...Object.entries(code).map(([name, color]) => `--code-${name}: ${color};`),
    ];
    return `color-scheme: ${scheme}; ${entries.join(' ')}`;
  };
  return [
    `:root { ${variables('light')} }`,
    `@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) { ${variables('dark')} } }`,
    `:root[data-theme="dark"] { ${variables('dark')} }`,
  ].join('\n');
}

const COMPILER = 'target/wasm32-wasip1/dist/vun.wasm';
// How many hexadecimal digits of the compiler's hash name its build.
const BUILD_DIGITS = 16;

const MODULES: Record<string, () => string> = {
  toolchain: () => `export default ${JSON.stringify(createHash('sha256').update(bytes(COMPILER)).digest('hex').slice(0, BUILD_DIGITS))};`,
  'brand.css': brand,
  highlight: () => `export default ${JSON.stringify(highlighting())};`,
  lessons: () => `export default ${JSON.stringify(lessons())};`,
};

const LIBRARY = 'lib';

// The files served beside the pages, by their path there.
const ASSETS: Record<string, () => Buffer | string> = {
  'play/vun.wasm': () => bytes(COMPILER),
  'play/library.json': () => JSON.stringify(Object.fromEntries(files(LIBRARY).map((path) => [path, source(`${LIBRARY}/${path}`)]))),
};

const TYPES: Record<string, string> = {'.wasm': 'application/wasm', '.json': 'application/json'};

const PREFIX = 'virtual:';

// The directories a change in which makes the virtual modules again.
const WATCHED = ['assets/', 'site/lessons/', 'editors/vscode/syntaxes/', 'target/wasm32-wasip1/dist/'];

export function site(): Plugin {
  let base = '/';
  return {
    name: 'vun-site',
    configResolved: (config) => void (base = config.base),
    resolveId: (asked) => (asked.startsWith(PREFIX) ? `\0${asked}` : undefined),
    load: (id) => (id.startsWith(`\0${PREFIX}`) ? MODULES[id.slice(PREFIX.length + 1)]?.() : undefined),
    generateBundle() {
      for (const [fileName, made] of Object.entries(ASSETS)) this.emitFile({type: 'asset', fileName, source: made()});
    },
    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        const path = request.url?.split('?')[0].slice(base.length) ?? '';
        const made = ASSETS[path];
        if (made === undefined) return next();
        response.setHeader('Content-Type', TYPES[path.slice(path.lastIndexOf('.'))]);
        response.end(made());
      });
      server.watcher.add(WATCHED.map((dir) => `${REPOSITORY_DIR}${dir}`));
      server.watcher.on('change', (file) => {
        if (!WATCHED.some((dir) => file.startsWith(`${REPOSITORY_DIR}${dir}`))) return;
        for (const module of server.moduleGraph.idToModuleMap.values()) {
          if (module.id?.startsWith(`\0${PREFIX}`)) server.moduleGraph.invalidateModule(module);
        }
        server.ws.send({type: 'full-reload'});
      });
    },
  };
}
