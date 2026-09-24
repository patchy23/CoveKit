import { createApp } from 'vue'
import { createPinia } from 'pinia'
import NativeEditorApp from './NativeEditorApp.vue'
import { i18n } from '@/i18n'
import { installErrorCollectors } from '@/core/diagnostics'
import '@/assets/styles/main.css'
const app = createApp(NativeEditorApp)
app.use(createPinia()).use(i18n)
installErrorCollectors(app)
app.mount('#app')
