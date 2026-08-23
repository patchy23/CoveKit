import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import '@/plugins' // 插件引导（副作用导入，各自注册 manifest）
import '@/assets/styles/main.css'

// i18n 基建（架构：vue-i18n，zh-CN 默认；en-US 文案 M4 迁移）
import { i18n } from '@/i18n'

createApp(App).use(createPinia()).use(i18n).mount('#app')
