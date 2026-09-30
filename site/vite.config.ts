import {fileURLToPath} from 'node:url';

import {defineConfig} from 'vite';

import {site} from './build/plugin';
import {REPOSITORY_DIR} from './build/source';

const ISOLATED = {'Cross-Origin-Opener-Policy': 'same-origin', 'Cross-Origin-Embedder-Policy': 'require-corp'};

export default defineConfig({
  base: process.env.VUN_SITE_BASE ?? '/',
  plugins: [site()],
  // Isolated across origins, so that the language server's worker can wait on shared memory.
  server: {fs: {allow: [REPOSITORY_DIR]}, headers: ISOLATED},
  preview: {headers: ISOLATED},
  // The brand's timings are read from `assets/motion`, whose tsconfig extends a package installed
  // there only: its sources compile with this project's options.
  esbuild: {tsconfigRaw: JSON.stringify({compilerOptions: {target: 'es2022', useDefineForClassFields: true}})},
  build: {
    rollupOptions: {
      input: {
        index: fileURLToPath(new URL('index.html', import.meta.url)),
        play: fileURLToPath(new URL('play.html', import.meta.url)),
      },
    },
  },
});
