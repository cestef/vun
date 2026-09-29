import {fileURLToPath} from 'node:url';

import ffmpeg from '@motion-canvas/ffmpeg';
import motionCanvas from '@motion-canvas/vite-plugin';
import {defineConfig} from 'vite';

// The scenes read the repository's sources: the palette and the logo.
const repository = fileURLToPath(new URL('../..', import.meta.url));

export default defineConfig({
  plugins: [motionCanvas({project: ['./src/sting.ts', './src/sting-mascot.ts', './src/mascot.ts']}), ffmpeg()],
  server: {fs: {allow: [repository]}},
});
