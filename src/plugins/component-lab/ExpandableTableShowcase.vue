<script setup lang="ts">
/** 行展开的本地演示，覆盖嵌套表格、表单、反馈状态与内容保留策略。 */
import { defineComponent, h, ref } from 'vue'
import {
  UiButton,
  UiCheckbox,
  UiEmptyState,
  UiInput,
  UiPanel,
  UiSelect,
  UiTable,
  UiTableCell,
  UiTableExpandableRow,
} from '@/core/ui'

const single = ref(true)
const keepMounted = ref(false)
const opened = ref(['orders'])
const state = ref('ready')
const DemoNote = defineComponent({
  setup() {
    const note = ref('')
    return () =>
      h(UiInput, {
        'aria-label': '演示备注',
        placeholder: '输入备注，收起再展开观察是否保留',
        modelValue: note.value,
        'onUpdate:modelValue': (value: string | number) => {
          note.value = String(value)
        },
      })
  },
})
const options = [
  { value: 'ready', label: '正常内容' },
  { value: 'loading', label: '加载中' },
  { value: 'empty', label: '空数据' },
  { value: 'error', label: '读取失败' },
]
const rows = [
  { id: 'orders', name: '订单服务', description: '嵌套子表格', disabled: false },
  { id: 'settings', name: '通知设置', description: '内嵌表单', disabled: false },
  { id: 'locked', name: '不可用项目', description: '禁用展开', disabled: true },
]
function expand(id: string, value: boolean) {
  opened.value = value
    ? single.value
      ? [id]
      : [...opened.value, id]
    : opened.value.filter((key) => key !== id)
}
function changeSingle(value: boolean) {
  single.value = value
  if (value) opened.value = opened.value.slice(0, 1)
}
</script>

<template>
  <UiPanel
    title="表格行展开"
    description="名称前的箭头与行尾按钮共用展开状态。子区域可放表格或表单；数据请求和反馈由业务负责。"
  >
    <div class="mb-md flex flex-wrap items-center gap-md">
      <UiCheckbox :model-value="single" label="一次展开一行" @update:model-value="changeSingle" />
      <UiCheckbox v-model="keepMounted" label="收起保留内容" />
      <UiSelect
        v-model="state"
        :options="options"
        size="sm"
        class="w-[140px]"
        aria-label="演示数据状态"
      />
    </div>
    <UiTable density="compact" striped>
      <thead>
        <tr>
          <UiTableCell as="th">项目</UiTableCell
          ><UiTableCell as="th">内容类型</UiTableCell
          ><UiTableCell as="th" align="right">操作</UiTableCell>
        </tr>
      </thead>
      <tbody>
        <UiTableExpandableRow
          v-for="row in rows"
          :key="row.id"
          :expanded="opened.includes(row.id)"
          :columns="3"
          :label="row.name"
          :disabled="row.disabled"
          :keep-mounted="keepMounted"
          @update:expanded="expand(row.id, $event)"
        >
          <template #default="{ toggle, expanded, detailsId }">
            <UiTableCell>{{ row.description }}</UiTableCell>
            <UiTableCell content="action" align="right"
              ><UiButton
                size="xs"
                variant="ghost"
                :disabled="row.disabled"
                :aria-expanded="expanded"
                :aria-controls="detailsId"
                @click="toggle"
                >{{ expanded ? '收起' : '查看' }}</UiButton
              ></UiTableCell
            >
          </template>
          <template #details>
            <p
              v-if="state === 'loading'"
              role="status"
              class="px-sm py-md text-body-sm text-secondary dark:text-secondary-dark"
            >
              正在读取数据…
            </p>
            <div
              v-else-if="state === 'error'"
              role="alert"
              class="select-text flex items-center gap-sm px-sm py-md text-body-sm text-danger-strong dark:text-danger-dark"
            >
              读取失败，请重试。<UiButton size="xs" @click="state = 'ready'">重试</UiButton>
            </div>
            <UiEmptyState v-else-if="state === 'empty'" compact title="暂无子项" />
            <UiTable v-else-if="row.id === 'orders'" density="compact" :framed="false">
              <thead>
                <tr>
                  <UiTableCell as="th">实例</UiTableCell
                  ><UiTableCell as="th">状态</UiTableCell
                  ><UiTableCell as="th">运行时间</UiTableCell>
                </tr>
              </thead>
              <tbody>
                <tr v-for="instance in ['orders-web-1', 'orders-worker-1']" :key="instance">
                  <UiTableCell content="technical">{{ instance }}</UiTableCell
                  ><UiTableCell>运行中</UiTableCell
                  ><UiTableCell>2 小时</UiTableCell>
                </tr>
              </tbody>
            </UiTable>
            <div v-else class="flex max-w-[420px] flex-col gap-sm p-sm">
              <DemoNote />
              <p class="text-caption text-text-muted dark:text-text-muted-dark">
                表单内部状态随内容卸载而清空，选择保留后收起不卸载。
              </p>
            </div>
          </template>
        </UiTableExpandableRow>
      </tbody>
    </UiTable>
  </UiPanel>
</template>
