<script setup lang="ts">
/**
 * PickerView · 屏幕取色遮罩页（picker 窗口通过 #/picker 路由加载本页）
 * 半透明全屏遮罩 + 鼠标移动实时显示所在像素色号，点击确定回传主窗口并关闭。
 */
import { onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ipc } from "@/core/ipc/ipc";

const win = getCurrentWindow();
const color = ref("#000000");
const picking = ref(false);
const lastPick = ref(0);

/** 鼠标移动取色（100ms 节流，避免 IPC 过频） */
async function onMove() {
  const now = Date.now();
  if (now - lastPick.value < 100) return;
  lastPick.value = now;
  const r = await ipc.colorPickScreen().catch(() => null);
  if (r && r.hex) color.value = r.hex;
}

/** 点击确定：回传颜色 → 关闭取色窗口 */
function onPick() {
  win.emit("picker-color", color.value);
  win.close();
}

/** Esc 取消 */
function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") win.close();
}

onMounted(() => {
  picking.value = true;
  window.addEventListener("keydown", onKey);
});
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <div
    class="h-screen w-screen cursor-crosshair select-none bg-black/40"
    @mousemove="onMove"
    @click="onPick"
  >
    <!-- 悬浮色号卡 -->
    <div
      class="pointer-events-none fixed left-[16px] top-[16px] flex items-center gap-[12px] rounded-lg bg-black/70 px-[16px] py-[10px] shadow-[0_8px_24px_rgba(0,0,0,0.35)]"
    >
      <span
        class="h-[28px] w-[28px] rounded-md border border-white/40"
        :style="{ backgroundColor: color }"
      />
      <span class="font-mono text-body text-white">{{ color }}</span>
    </div>

    <!-- 底部提示 -->
    <div
      class="pointer-events-none fixed bottom-[20px] left-1/2 -translate-x-1/2 rounded-full bg-black/70 px-[16px] py-[8px] text-body-sm text-white/90"
    >
      移动鼠标取色 · 点击确定 · Esc 取消
    </div>
  </div>
</template>
