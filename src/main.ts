import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import '@/plugins' // 插件引导（副作用导入，各自注册 manifest）
import '@/assets/styles/main.css'

// i18n 基建（架构：vue-i18n，zh-CN 默认；en-US 文案 M4 迁移）
import { i18n } from '@/i18n'
import { installErrorCollectors } from '@/core/diagnostics'
import { collectAllToolBlockers, requestAppExit, scopeStats } from '@/core/lifecycle'

/**
 * 开发构建专用取数钩子（不进正式版：`import.meta.env.DEV` 生产构建静态替换为 false）。
 *
 * - `__pbScopeStats`：页签快速开关后读回活体作用域/订阅/定时器计数（可靠性 T10）；
 * - `__pbExit`：托盘菜单退出在走查环境里点不到（图标落在溢出区），
 *   用它复现同一条退出协商链路（同 `requestAppExit`，含页面 owner 阻断者收集）。
 */
if (import.meta.env.DEV) {
  const hooks = {
    __pbScopeStats: scopeStats,
    __pbExit: { collectAllToolBlockers, requestAppExit },
  }
  Object.assign(window as unknown as Record<string, unknown>, hooks)
}

const app = createApp(App)
app.use(createPinia()).use(i18n)
// 全局错误留档：渲染异常与未处理的 Promise 拒绝进诊断清单，不再只闪一个 toast
installErrorCollectors(app)
app.mount('#app')
