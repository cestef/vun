import {fileURLToPath} from 'node:url';

import ffmpeg from '@motion-canvas/ffmpeg';
import motionCanvas from '@motion-canvas/vite-plugin';
import {defineConfig} from 'vite';

// The scenes read the repository's sources: the palette, the logo, the code colors, the grammar
// the editors use and the corpus.
const repository = fileURLToPath(new URL('../..', import.meta.url));

const projects = [
  'sting',
  'sting-mascot',
  'sting-short',
  'mascot',
  'clip',
  'clip-square',
  'clip-tall',
  'lower-third',
  'talk',
  'release',
];

export default defineConfig({
  plugins: [motionCanvas({project: projects.map((name) => `./src/${name}.ts`)}), ffmpeg()],
  server: {fs: {allow: [repository]}},
});
