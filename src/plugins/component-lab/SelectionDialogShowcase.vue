<script setup lang="ts">
/** 公共选择与复合弹窗检查入口，名称与统一导出保持一致。 */
import { ref } from 'vue'
import {
  UiPanel,
  UiField,
  UiButton,
  UiSlider,
  UiKeyValueEditor,
  UiTreeSelect,
  UiCombobox,
  UiConfirmDialog,
  UiInputDialog,
  type UiKeyValueRow,
} from '@/core/ui'
const value = ref(40),
  selected = ref('dev/users'),
  combo = ref('http')
const rows = ref<UiKeyValueRow[]>([{ id: 'example', key: 'Accept', value: 'application/json' }])
const confirmOpen = ref(false),
  inputOpen = ref(false),
  result = ref('等待操作')
const options = [
  {
    value: 'dev',
    label: '开发',
    children: [
      { value: 'dev/users', label: '用户' },
      { value: 'dev/orders', label: '订单' },
    ],
  },
  { value: 'prod', label: '生产', disabled: true },
]
function confirmExample() {
  result.value = '已确认'
  confirmOpen.value = false
}
function inputExample(value: string) {
  result.value = `输入：${value}`
  inputOpen.value = false
}
</script>
<template>
  <UiPanel title="选择与复合弹窗" description="检查新名称对应的真实控件、键盘操作和弹窗反馈。">
    <div class="grid gap-md lg:grid-cols-2">
      <UiField label="UiSlider · 单值滑杆" :description="`当前值：${value}`"
        ><UiSlider v-model="value"
      /></UiField>
      <UiField label="UiCombobox · 搜索选择"
        ><UiCombobox
          v-model="combo"
          :options="[
            { value: 'http', label: 'HTTP' },
            { value: 'ws', label: 'WebSocket' },
          ]"
      /></UiField>
      <UiField label="UiTreeSelect · 树形选择"
        ><UiTreeSelect v-model="selected" :options="options" label="示例分组"
      /></UiField>
      <UiField label="UiKeyValueEditor · 键值编辑"
        ><UiKeyValueEditor v-model:rows="rows"
      /></UiField>
    </div>
    <div class="mt-md flex flex-wrap items-center gap-sm">
      <UiButton size="sm" @click="confirmOpen = true">UiConfirmDialog · 确认弹窗</UiButton>
      <UiButton size="sm" @click="inputOpen = true">UiInputDialog · 输入弹窗</UiButton>
      <span role="status" class="text-body-sm text-secondary dark:text-secondary-dark">{{
        result
      }}</span>
    </div>
    <UiConfirmDialog
      :open="confirmOpen"
      title="确认示例"
      message="这是组件检查示例，不会操作业务数据。"
      @close="confirmOpen = false"
      @confirm="confirmExample"
    />
    <UiInputDialog
      :open="inputOpen"
      title="输入示例"
      label="示例名称"
      initial-value="示例"
      @close="inputOpen = false"
      @confirm="inputExample"
    />
  </UiPanel>
</template>
