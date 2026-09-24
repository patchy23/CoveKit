// ESLint 9 flat config（项目规范：质量类规则，格式交给 Prettier）
import js from '@eslint/js'
import pluginVue from 'eslint-plugin-vue'
import tseslint from 'typescript-eslint'
import prettierConfig from 'eslint-config-prettier/flat'
import globals from 'globals'

export default tseslint.config(
  {
    ignores: [
      'dist/**',
      'src-tauri/**',
      'node_modules/**',
      'coverage/**',
      '.work/**',
      'playwright-report/**',
      'test-results/**',
    ],
  },
  {
    files: ['src/**/*.{js,mjs,ts,mts,tsx,vue}', 'tests/**/*.{js,mjs,ts,mts,tsx,vue}'],
    // 应用与 DOM 测试使用浏览器环境。
    languageOptions: {
      globals: { ...globals.browser },
    },
  },
  {
    files: ['*.config.{js,mjs,ts,mts}', 'scripts/**/*.{js,mjs,ts,mts}'],
    languageOptions: {
      globals: { ...globals.node },
    },
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...pluginVue.configs['flat/recommended'],
  {
    files: ['**/*.vue'],
    languageOptions: {
      parserOptions: {
        parser: tseslint.parser,
      },
    },
  },
  {
    rules: {
      // Vue 组件名不强制多词（App.vue 等单名组件）
      'vue/multi-word-component-names': 'off',
      // 未使用组件阻止检查通过，与命令的零警告要求保持一致。
      'vue/no-unused-components': 'error',
      'vue/no-v-html': 'error',
    },
  },
  {
    files: ['src/features/ui/AppIcon.vue'],
    rules: {
      // 此组件仅渲染仓库内静态图标表，不接受外部 HTML。
      'vue/no-v-html': 'off',
    },
  },
  // 关闭与 Prettier 冲突的格式规则（格式统一交给 Prettier）
  prettierConfig
)
