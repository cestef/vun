import {defineConfig} from 'vitest/config';

// The tests run the compiler built for WASI, which takes a while the first time.
export default defineConfig({test: {include: ['tests/**/*.test.ts'], testTimeout: 120_000}});
