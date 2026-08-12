<script setup lang="ts">
import { reactive, ref } from 'vue'
import { UiButton, UiField, UiInput, UiModal, UiPagination, UiPanel, UiTabs } from '@/core/ui'
import type { UiSize } from '@/core/ui'

const sizes: UiSize[] = ['xs', 'sm', 'md', 'lg']
const tabs = [
  { value: 'overview', label: '概览' },
  { value: 'records', label: '记录', badge: 12 },
  { value: 'settings', label: '设置' },
]
const activeTabs = reactive<Record<UiSize, string>>({
  xs: 'overview',
  sm: 'records',
  md: 'overview',
  lg: 'settings',
})
const page = ref(4)
const modalOpen = ref(false)
const modalSize = ref<'sm' | 'md' | 'lg' | 'xl'>('md')

function openModal(size: 'sm' | 'md' | 'lg' | 'xl') {
  modalSize.value = size
  modalOpen.value = true
}
</script>

<template>
  <UiPanel
    title="页签与分段导航"
    description="pill 用于同级视图，line 用于工作区内部功能；两者均有四档尺寸。"
  >
    <div class="flex flex-col gap-md">
      <UiTabs
        v-for="size in sizes"
        :key="`pill-${size}`"
        v-model="activeTabs[size]"
        :items="tabs"
        :size="size"
      />
      <div class="overflow-hidden rounded-lg border border-border dark:border-border-dark">
        <UiTabs v-model="activeTabs.md" :items="tabs" variant="line" />
        <div class="p-md text-body-sm text-secondary dark:text-secondary-dark">
          当前内容：{{ tabs.find((item) => item.value === activeTabs.md)?.label }}
        </div>
      </div>
    </div>
  </UiPanel>

  <div class="grid gap-md lg:grid-cols-2">
    <UiPanel title="分页" description="长列表使用受控页码，默认 sm 适配数据工具。">
      <UiPagination v-model="page" :total-pages="12" />
      <p class="mt-md text-body-sm text-text-muted dark:text-text-muted-dark">
        当前第 {{ page }} 页
      </p>
    </UiPanel>

    <UiPanel title="弹窗尺寸" description="弹窗按内容复杂度提供 sm / md / lg / xl。">
      <div class="flex flex-wrap gap-sm">
        <UiButton size="sm" @click="openModal('sm')">小弹窗</UiButton>
        <UiButton size="sm" @click="openModal('md')">标准弹窗</UiButton>
        <UiButton size="sm" @click="openModal('lg')">大弹窗</UiButton>
        <UiButton size="sm" @click="openModal('xl')">超大弹窗</UiButton>
      </div>
    </UiPanel>
  </div>

  <UiModal
    :open="modalOpen"
    :size="modalSize"
    title="保存连接配置"
    :description="`当前尺寸：${modalSize}`"
    @close="modalOpen = false"
  >
    <UiField label="配置名称" required><UiInput model-value="本地开发数据库" /></UiField>
    <template #footer>
      <UiButton variant="ghost" @click="modalOpen = false">取消</UiButton>
      <UiButton variant="primary" @click="modalOpen = false">保存</UiButton>
    </template>
  </UiModal>
</template>
