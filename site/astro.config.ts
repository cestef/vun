import starlight from '@astrojs/starlight';
import {fileURLToPath} from 'node:url';

import solid from '@solidjs/vite-plugin';
import {defineConfig} from 'astro/config';

import {brandCss} from './build/brand';
import {build} from './build/compiler';
import {highlighting} from './build/highlight';
import {REPOSITORY_DIR} from './build/source';
import {version} from './build/version';
import {GUIDE} from './src/guide';

const ISOLATED = {'Cross-Origin-Opener-Policy': 'same-origin', 'Cross-Origin-Embedder-Policy': 'require-corp'};
const REPOSITORY = 'https://github.com/cestef/vun';
const highlight = highlighting();
const {grammar, themes} = highlight;

export default defineConfig({
  base: process.env.VUN_SITE_BASE ?? '/',
  redirects: {'/learn': '/docs/learn/'},
  devToolbar: {enabled: false},
  integrations: [
    starlight({
      title: 'vun',
      description: 'The guide to Vun, a smol systems programming language.',
      favicon: '/icon.svg',
      tableOfContents: {minHeadingLevel: 2, maxHeadingLevel: 3},
      editLink: {baseUrl: `${REPOSITORY}/edit/main/site/`},
      head: [{tag: 'style', content: brandCss()}],
      customCss: ['./src/styles/tokens.css', './src/styles/controls.css', './src/styles/shell.css', './src/styles/docs.css'],
      components: {
        ThemeProvider: './src/components/ThemeInit.astro',
        ThemeSelect: './src/components/ThemeButton.astro',
        Header: './src/components/DocsHeader.astro',
        SiteTitle: './src/components/Nothing.astro',
        MarkdownContent: './src/components/Content.astro',
      },
      sidebar: GUIDE.map(({label, pages, directory}) => ({label, items: pages ?? [{autogenerate: {directory: directory!}}]})),
      expressiveCode: {
        themes: themes.map((theme) => ({...theme, type: theme.name})) as never,
        shiki: {langs: [{...grammar, name: 'vun'} as never]},
        useStarlightDarkModeSwitch: true,
        defaultProps: {wrap: true, preserveIndent: true, overridesByLang: {'sh,shell,bash,ansi': {frame: 'none'}}},
        styleOverrides: {
          borderRadius: '16px',
          codeFontFamily: "'JetBrains Mono Variable', ui-monospace, monospace",
          codeFontSize: '0.8125rem',
          codeLineHeight: '1.7',
          codePaddingBlock: '1rem',
          codePaddingInline: '1.25rem',
          uiFontFamily: "'JetBrains Mono Variable', ui-monospace, monospace",
          frames: {shadowColor: 'transparent'},
        },
      },
    }),
  ],
  vite: {
    plugins: [solid()],
    define: {__TOOLCHAIN__: JSON.stringify(build()), __VERSION__: JSON.stringify(version()), __HIGHLIGHT__: JSON.stringify(highlight)},
    server: {fs: {allow: [REPOSITORY_DIR]}, headers: ISOLATED},
    preview: {headers: ISOLATED},
    // The brand's timings are read from `assets/motion`, whose tsconfig extends a package installed there only: its sources compile with this project's.
    tsconfig: fileURLToPath(new URL('./tsconfig.json', import.meta.url)),
  },
});
