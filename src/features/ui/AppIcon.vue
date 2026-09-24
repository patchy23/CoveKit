<script setup lang="ts">
/**
 * AppIcon · 全局线性图标组件（对齐原型 002 图标集）
 * 图标 key 由 ToolManifest.icon 与各处 UI 引用；后续新增图标只需在此表加一行。
 */
defineProps<{ name: string; size?: number | string }>()

const inner: Record<string, string> = {
  github:
    '<path d="M9 19c-4.3 1.3-4.3-2.2-6-2.7M15 22v-3.9a3.4 3.4 0 0 0-.9-2.7c3-.3 6.2-1.5 6.2-6.9A5.4 5.4 0 0 0 18.8 5a5 5 0 0 0-.1-3.5S17.5 1.2 15 2.8a13.4 13.4 0 0 0-7 0C5.5 1.2 4.3 1.5 4.3 1.5A5 5 0 0 0 4.2 5a5.4 5.4 0 0 0-1.5 3.8c0 5.4 3.2 6.6 6.2 6.9a3.4 3.4 0 0 0-.9 2.6V22"/>',
  // 导航
  all: '<rect x="3.5" y="3.5" width="7" height="7" rx="1.8"/><rect x="13.5" y="3.5" width="7" height="7" rx="1.8"/><rect x="3.5" y="13.5" width="7" height="7" rx="1.8"/><rect x="13.5" y="13.5" width="7" height="7" rx="1.8"/>',
  dev: '<path d="M8.5 6 4 12l4.5 6M15.5 6 20 12l-4.5 6"/>',
  text: '<path d="M5 7h14M5 12h9M5 17h14"/>',
  image:
    '<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><circle cx="9" cy="10" r="1.6"/><path d="M4 17l5-4 3.5 3 3.5-3.5L20 17"/>',
  net: '<circle cx="12" cy="12" r="8.5"/><path d="M3.5 12h17M12 3.5c2.4 2.4 3.8 5.3 3.8 8.5s-1.4 6.1-3.8 8.5c-2.4-2.4-3.8-5.3-3.8-8.5s1.4-6.1 3.8-8.5z"/>',
  sys: '<path d="M4 7h10M18 7h2M4 17h2M10 17h10"/><circle cx="16" cy="7" r="2.2"/><circle cx="8" cy="17" r="2.2"/>',
  fav: '<path d="M12 3.5l2.6 5.5 6 .7-4.4 4.2 1.2 6L12 17l-5.4 2.9 1.2-6L3.4 9.7l6-.7z"/>',

  // 顶栏 / 通用
  search: '<circle cx="11" cy="11" r="7"/><path d="M16.5 16.5 21 21"/>',
  grid: '<rect x="3.5" y="3.5" width="7" height="7" rx="1.8"/><rect x="13.5" y="3.5" width="7" height="7" rx="1.8"/><rect x="3.5" y="13.5" width="7" height="7" rx="1.8"/><rect x="13.5" y="13.5" width="7" height="7" rx="1.8"/>',
  list: '<path d="M9 6h11M9 12h11M9 18h11M4 6h.01M4 12h.01M4 18h.01"/>',
  add: '<path d="M12 5v14M5 12h14"/>',
  moon: '<path d="M12 3.5a8.5 8.5 0 1 0 8.5 8.5c-4.5 0-8.5-4-8.5-8.5z"/>',
  sun: '<circle cx="12" cy="12" r="4.5"/><path d="M12 2.8v2M12 19.2v2M2.8 12h2M19.2 12h2M5.4 5.4l1.4 1.4M17.2 17.2l1.4 1.4M18.6 5.4l-1.4 1.4M6.8 17.2l-1.4 1.4"/>',
  gear: '<circle cx="12" cy="12" r="3.2"/><path d="M12 2.8v2.3M12 18.9v2.3M2.8 12h2.3M18.9 12h2.3M5.4 5.4l1.6 1.6M17 17l1.6 1.6M18.6 5.4 17 7M7 17l-1.6 1.6"/>',
  sliders: '<path d="M21 4h-7M10 4H3M21 12h-9M8 12H3M21 20h-5M12 20H3M14 2v4M8 10v4M16 18v4"/>',
  close: '<path d="M18 6 6 18M6 6l12 12"/>',
  minus: '<path d="M5 12h14"/>',
  maximize: '<rect x="5" y="5" width="14" height="14" rx="1.5"/>',
  panel: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9.5 4v16"/>',
  immersive: '<path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/>',
  copy: '<rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>',
  check: '<path d="M20 6 9 17l-5-5"/>',
  run: '<path d="M5 12h14M12 5l7 7-7 7"/>',
  trash:
    '<path d="M3 6h18M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/>',

  // 工具图标（8 个第一批 + 预留）
  json: '<path d="M8.5 3.5H6a2 2 0 0 0-2 2V8c0 1.4-1 2.3-2 2.6 1 .3 2 1.2 2 2.6v2.8a2 2 0 0 0 2 2h2.5M15.5 3.5H18a2 2 0 0 1 2 2V8c0 1.4 1 2.3 2 2.6-1 .3-2 1.2-2 2.6v2.8a2 2 0 0 1-2 2h-2.5"/>',
  ts: '<circle cx="12" cy="12" r="8.5"/><path d="M12 7.5V12l3 2.2"/>',
  b64: '<path d="M4 7h12M13 4l3 3-3 3M20 17H8M11 14l-3 3 3 3"/>',
  url: '<path d="M10 14a4.5 4.5 0 0 0 6.4 0l3-3a4.5 4.5 0 0 0-6.4-6.4L11.8 6"/><path d="M14 10a4.5 4.5 0 0 0-6.4 0l-3 3a4.5 4.5 0 0 0 6.4 6.4l1.2-1.4"/>',
  stat: '<path d="M6 20v-5M12 20V9M18 20v-8M20 20H4"/>',
  diff: '<path d="M8 4H5.5A1.5 1.5 0 0 0 4 5.5v13A1.5 1.5 0 0 0 5.5 20H8M16 4h2.5A1.5 1.5 0 0 1 20 5.5v13a1.5 1.5 0 0 1-1.5 1.5H16"/><path d="M8 8h3.5M8 12h3.5M16 8h.01M16 12h.01"/>',
  md: '<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><path d="M7 15V9l2.5 3L12 9v6M15.5 9h3.5v6M15.5 12h3.5"/>',
  regex:
    '<path d="M5 5l14 14"/><circle cx="8.2" cy="15.8" r="1.6"/><circle cx="15.8" cy="8.2" r="1.6"/>',

  // 第二批轻量工具图标
  lock: '<rect x="5" y="11" width="14" height="9" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/>',
  hash: '<path d="M4 9h16M4 15h16M10 3 8 21M16 3l-2 18"/>',
  color:
    '<circle cx="12" cy="12" r="9"/><path d="M12 3a9 9 0 0 1 0 18c-2.5 0-3.5-2-2.5-3.5S10 15 9 15c-2.2 0-3.5-2.5-1.5-4.5.8-.8.5-2-1-2.2A8.9 8.9 0 0 0 12 3z"/>',
  qr: '<rect x="4" y="4" width="7" height="7" rx="1.5"/><rect x="13" y="4" width="7" height="7" rx="1.5"/><rect x="4" y="13" width="7" height="7" rx="1.5"/><path d="M13 13h3v3h-3zM16 16h4v4h-4zM20 13h.01"/>',
  clipboard:
    '<rect x="5" y="4" width="14" height="17" rx="2"/><path d="M9 4a3 3 0 0 1 6 0M9 12h6M9 16h4"/>',
  pin: '<path d="M9 4h6l-1 6 3.5 3.5H6.5L10 10z"/><path d="M12 13.5V20"/>',
  db: '<ellipse cx="12" cy="5.5" rx="8" ry="2.8"/><path d="M4 5.5v13c0 1.5 3.6 2.8 8 2.8s8-1.3 8-2.8v-13"/><path d="M4 12c0 1.5 3.6 2.8 8 2.8s8-1.3 8-2.8"/>',
  hosts:
    '<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><path d="M3.5 9h17M8 9v10.5M16 9v10.5"/>',
  'file-lock':
    '<path d="M11 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h8l4 4v3M12 3v5h5M6 12h4M6 16h3"/><rect x="13" y="15" width="8" height="6" rx="1.5"/><path d="M15 15v-2a2 2 0 0 1 4 0v2"/>',
  'port-viewer':
    '<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M7 8h3M7 12h6M16 8h1M9 20h6M12 16v4"/>',
  dns: '<circle cx="12" cy="12" r="7.5"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3"/><circle cx="12" cy="12" r="2.2"/>',
  xml: '<path d="M8 6l-4 6 4 6M16 6l4 6-4 6M13.5 4l-3 16"/>',
  tts: '<path d="M11 5 6 9H3v6h3l5 4z"/><path d="M15.5 8.5a5 5 0 0 1 0 7M18.5 5.5a9 9 0 0 1 0 13"/>',
  // FRP 客户端：两侧端面 + 中间穿透箭头（隧道意象，原创）
  frp: '<rect x="2.5" y="8" width="4" height="8" rx="1.2"/><rect x="17.5" y="8" width="4" height="8" rx="1.2"/><path d="M8.6 12h4.6"/><path d="m11.6 9.6 2.5 2.4-2.5 2.4"/>',
}
</script>

<template>
  <svg
    :width="size ?? 20"
    :height="size ?? 20"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    v-html="inner[name] ?? ''"
  />
</template>
