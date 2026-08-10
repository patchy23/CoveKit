// ESLint 9 flat config（项目规范：质量类规则，格式交给 Prettier）
import js from '@eslint/js'
import pluginVue from 'eslint-plugin-vue'
import tseslint from 'typescript-eslint'
import prettierConfig from 'eslint-config-prettier/flat'
import globals from 'globals'

export default tseslint.config(
  {
    ignores: ['dist/**', 'src-tauri/**', 'node_modules/**', 'coverage/**', '*.config.*'],
  },
  {
    // 浏览器环境全局（window/document 等）
    languageOptions: {
      globals: { ...globals.browser },
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
      // 模板中未使用组件仅提示（Vue SFC 中 auto-registered）
      'vue/no-unused-components': 'warn',
      // 图标表为静态常量（iconInner），v-html 无注入风险
      'vue/no-v-html': 'off',
    },
  },
  // 关闭与 Prettier 冲突的格式规则（格式统一交给 Prettier）
  prettierConfig
)
