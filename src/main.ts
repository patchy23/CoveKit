import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import '@/plugins' // 插件引导（副作用导入，各自注册 manifest）
import '@/assets/styles/main.css'

// i18n 基建（架构：vue-i18n，zh-CN 默认；en-US 文案 M4 迁移）
import { i18n } from '@/i18n'
import { installErrorCollectors } from '@/core/diagnostics'

const app = createApp(App)
app.use(createPinia()).use(i18n)
// 全局错误留档：渲染异常与未处理的 Promise 拒绝进诊断清单，不再只闪一个 toast
installErrorCollectors(app)
app.mount('#app')
