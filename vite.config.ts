import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'
import { fileURLToPath, URL } from 'node:url'

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [vue(), tailwindcss()],

  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ['**/src-tauri/**'],
    },
  },

  build: {
    rollupOptions: {
      output: {
        /**
         * 将稳定的大型依赖从应用入口拆开，降低主包下载与解析成本。
         *
         * 编辑器只把运行时核心归入 vendor-editor：语言包（@codemirror/lang-*）、language-data
         * 与各语言 @lezer 语法必须保持按需分包——它们是动态 import() 加载的，一旦强制并入
         * vendor-editor，每次启动都会下载全部语言（实测 572KB gzip）。
         */
        manualChunks(id) {
          if (!id.includes('node_modules')) return undefined
          const editorRuntime = [
            '@codemirror/state',
            '@codemirror/view',
            '@codemirror/commands',
            '@codemirror/language',
            '@codemirror/search',
            '@codemirror/autocomplete',
            '@lezer/common',
            '@lezer/highlight',
            '@lezer/lr',
          ]
          if (editorRuntime.some((pkg) => id.includes(`node_modules/${pkg}/`)))
            return 'vendor-editor'
          if (id.includes('@xterm') || id.includes('/xterm/')) return 'vendor-terminal'
          if (id.includes('highlight.js') || id.includes('/marked/')) return 'vendor-markdown'
          if (
            id.includes('/vue/') ||
            id.includes('/@vue/') ||
            id.includes('/pinia/') ||
            id.includes('/vue-i18n/') ||
            id.includes('/reka-ui/')
          ) {
            return 'vendor-vue'
          }
          if (id.includes('/@tauri-apps/')) return 'vendor-tauri'
          return undefined
        },
      },
    },
  },
}))
