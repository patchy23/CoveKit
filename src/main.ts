import { createApp } from "vue";
import { createPinia } from "pinia";
import { createI18n } from "vue-i18n";
import App from "./App.vue";
import "@/tools"; // 工具目录自注册引导（副作用导入）
import "@/assets/styles/main.css";

// i18n 基建（架构：vue-i18n，zh-CN 默认；en-US 文案 M4 迁移）
const i18n = createI18n({
  legacy: false,
  locale: "zh-CN",
  fallbackLocale: "zh-CN",
  messages: { "zh-CN": {} },
});

createApp(App).use(createPinia()).use(i18n).mount("#app");
