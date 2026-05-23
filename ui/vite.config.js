import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

const __dirname = dirname(fileURLToPath(import.meta.url));
const jsRoot = resolve(__dirname, '../assets/js');

export default defineConfig({
  build: {
    outDir: '.vite-check',
    emptyOutDir: true,
    lib: {
      entry: {
        preview: resolve(jsRoot, 'preview.js'),
        main: resolve(jsRoot, 'main.js'),
        gist: resolve(jsRoot, 'gist.js'),
      },
      formats: ['es'],
      fileName: (_, name) => `${name}.js`,
    },
    rollupOptions: {
      output: {
        entryFileNames: '[name].js',
        chunkFileNames: 'chunks/[name].js',
        assetFileNames: '[name].[ext]',
        manualChunks: (id) => {
          if (id.includes('/adapters/')) {
            return 'adapters';
          }
          if (
            id.includes('/dom.js') ||
            id.includes('/prefs.js') ||
            id.includes('/vendor.js') ||
            id.includes('/status.js') ||
            id.includes('/live.js')
          ) {
            return 'shared';
          }
          return undefined;
        },
      },
    },
    minify: false,
    sourcemap: false,
  },
});
