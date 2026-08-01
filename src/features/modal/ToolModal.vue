<script setup lang="ts">
/**
 * ToolModal · 工具弹窗载体（presentation=modal）
 * 按 manifest.presentation 路由（resolvePresentation），懒加载工具组件；
 * 工具卸载时清理副作用（第二批会话生命周期在此扩展）。
 */
import { computed, defineAsyncComponent, shallowRef, watch, type Component } from "vue";
import { getTool } from "@/core/registry/toolRegistry";
import BaseModal from "@/features/ui/BaseModal.vue";
import AppIcon from "@/features/ui/AppIcon.vue";
import { useUiStore } from "@/stores/ui";

const ui = useUiStore();

const manifest = computed(() => (ui.openToolId ? getTool(ui.openToolId) : undefined));

const ToolComponent = shallowRef<Component | null>(null);
watch(
  () => ui.openToolId,
  (id) => {
    const m = id ? getTool(id) : undefined;
    // defineAsyncComponent 包装懒加载工厂（manifest.component 为 () => import(...)）
    ToolComponent.value = m ? defineAsyncComponent(m.component) : null;
  },
  { immediate: true }
);

function close() {
  ui.openToolId = null;
}
</script>

<template>
  <BaseModal :open="Boolean(manifest)" @close="close">
    <template v-if="manifest">
      <div class="flex items-center gap-[12px]">
        <div
          class="grid h-[46px] w-[46px] shrink-0 place-items-center rounded-[12px] bg-tertiary-soft dark:bg-tertiary-soft-dark"
        >
          <AppIcon
            :name="manifest.icon"
            :size="23"
            class="text-tertiary-strong dark:text-tertiary-dark"
          />
        </div>
        <div>
          <h2 class="text-[17px] font-extrabold tracking-[-0.02em] dark:text-primary-dark">
            {{ manifest.name }}
          </h2>
          <p class="mt-[3px] text-[12.5px] text-secondary dark:text-secondary-dark">
            {{ manifest.description }}
          </p>
        </div>
        <button
          class="ml-auto grid h-8 w-8 shrink-0 cursor-pointer place-items-center rounded-[9px] bg-neutral text-[14px] text-secondary transition-colors duration-150 hover:bg-border hover:text-primary dark:bg-neutral-dark dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
          title="关闭"
          @click="close"
        >
          <AppIcon name="close" :size="14" />
        </button>
      </div>
      <div class="mt-[20px]">
        <component :is="ToolComponent" v-if="ToolComponent" />
      </div>
    </template>
  </BaseModal>
</template>
