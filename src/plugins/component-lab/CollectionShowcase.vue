<script setup lang="ts">
/** 公共列表和树的受控保存示例：失败时不替换原数据。 */
import { computed, onBeforeUnmount, ref } from 'vue'
import {
  UiButton,
  UiCheckbox,
  UiPanel,
  UiSortableList,
  UiTree,
  type UiCollectionMove,
  type UiListItem,
  type UiTreeItem,
} from '@/core/ui'
const items = ref<UiListItem[]>([
  { id: 'prepare', label: '准备环境', description: '检查变量和连接' },
  { id: 'request', label: '发送请求', badge: 'GET' },
  { id: 'verify', label: '校验结果' },
  { id: 'locked', label: '固定步骤', disabled: true },
])
const selected = ref('request')
const handle = ref(true),
  fail = ref(false),
  busy = ref(false)
const message = ref('拖动手柄排序，或聚焦行后按 Alt + ↑↓。')
const state = ref<'normal' | 'empty' | 'loading' | 'error'>('normal')
interface DemoNode {
  id: string
  label: string
  parent: string | null
  folder?: boolean
  disabled?: boolean
}
const nodes = ref<DemoNode[]>([
  { id: 'dev', label: '开发环境', parent: null, folder: true },
  { id: 'users', label: '用户服务', parent: 'dev', folder: true },
  { id: 'list', label: '用户列表', parent: 'users' },
  { id: 'empty', label: '空目录', parent: 'dev', folder: true },
  { id: 'prod', label: '生产环境', parent: null, folder: true },
  { id: 'fixed', label: '受保护目录', parent: null, folder: true, disabled: true },
  ...Array.from({ length: 18 }, (_, i) => ({
    id: `api-${i}`,
    label: `示例接口 ${i + 1}`,
    parent: 'prod',
  })),
])
const expanded = ref(new Set(['dev', 'users', 'prod']))
const treeSelected = ref('list')
const treeItems = computed(() => {
  const rows: UiTreeItem[] = []
  function visit(parent: string | null, depth: number) {
    for (const node of nodes.value.filter((node) => node.parent === parent)) {
      rows.push({ ...node, depth, expandable: node.folder, expanded: expanded.value.has(node.id) })
      if (node.folder && expanded.value.has(node.id)) visit(node.id, depth + 1)
    }
  }
  visit(null, 0)
  return rows
})
function toggle(item: UiTreeItem) {
  const next = new Set(expanded.value)
  if (next.has(item.id)) next.delete(item.id)
  else next.add(item.id)
  expanded.value = next
}
let timer: ReturnType<typeof setTimeout> | undefined
function save(change: () => void) {
  if (busy.value) return
  busy.value = true
  timer = setTimeout(() => {
    if (fail.value) message.value = '模拟保存失败：原顺序保持不变，可以重试。'
    else {
      change()
      message.value = '位置已保存；仅更新数据，不重新创建节点。'
    }
    busy.value = false
  }, 450)
}
function moveList(move: UiCollectionMove) {
  save(() => {
    const item = items.value.find((item) => item.id === move.id)!
    const next = items.value.filter((item) => item.id !== move.id)
    const index =
      move.targetId === null
        ? next.length
        : next.findIndex((item) => item.id === move.targetId) + Number(move.position === 'after')
    next.splice(index, 0, item)
    items.value = next
  })
}
function moveTree(move: UiCollectionMove) {
  save(() => {
    const source = nodes.value.find((node) => node.id === move.id)!
    const target = nodes.value.find((node) => node.id === move.targetId)
    const parent = move.position === 'inside' ? (target?.id ?? null) : (target?.parent ?? null)
    const next = nodes.value.filter((node) => node.id !== source.id)
    const index =
      move.position === 'inside' || !target
        ? next.length
        : next.findIndex((node) => node.id === target.id) + Number(move.position === 'after')
    next.splice(index, 0, { ...source, parent })
    nodes.value = next
    if (parent) expanded.value = new Set([...expanded.value, parent])
  })
}
onBeforeUnmount(() => clearTimeout(timer))
</script>
<template>
  <UiPanel
    title="可排序列表与可拖拽树"
    description="列表处理顺序，树额外处理层级；行前后显示插入线，目录中间高亮表示移入。"
  >
    <div class="mb-sm flex flex-wrap items-center gap-sm">
      <UiCheckbox v-model="handle" label="列表使用拖动手柄" />
      <UiCheckbox v-model="fail" label="模拟保存失败" />
      <UiButton
        v-for="option in [
          { value: 'normal', label: '正常' },
          { value: 'empty', label: '空数据' },
          { value: 'loading', label: '加载' },
          { value: 'error', label: '错误重试' },
        ] as const"
        :key="option.value"
        size="xs"
        :variant="state === option.value ? 'primary' : 'ghost'"
        @click="state = option.value"
        >{{ option.label }}</UiButton
      >
    </div>
    <div class="grid gap-md lg:grid-cols-2">
      <div class="flex h-[300px] flex-col rounded-md border border-border dark:border-border-dark">
        <p
          class="border-b border-border px-sm py-xs text-body-sm font-medium dark:border-border-dark"
        >
          可排序列表
        </p>
        <UiSortableList
          v-model="selected"
          :items="state === 'normal' ? items : []"
          :drag-handle="handle"
          :busy="busy"
          :loading="state === 'loading'"
          :error="state === 'error' ? '示例读取失败' : ''"
          :row-height="32"
          @move="moveList"
          @retry="state = 'normal'"
          @contextmenu="(item) => (message = `右键：${item.label}`)"
        />
      </div>
      <div class="flex h-[300px] flex-col rounded-md border border-border dark:border-border-dark">
        <p
          class="border-b border-border px-sm py-xs text-body-sm font-medium dark:border-border-dark"
        >
          可拖拽树 · 多层与空目录
        </p>
        <UiTree
          v-model="treeSelected"
          :items="state === 'normal' ? treeItems : []"
          draggable
          :busy="busy"
          :row-height="28"
          :loading="state === 'loading'"
          :error="state === 'error' ? '示例读取失败' : ''"
          @toggle="toggle"
          @move="moveTree"
          @retry="state = 'normal'"
        />
      </div>
    </div>
    <p role="status" class="mt-sm text-body-sm text-secondary dark:text-secondary-dark">
      {{ message }}
    </p>
    <p class="mt-xs text-caption text-text-muted dark:text-text-muted-dark">
      树支持 Alt + ← 移出当前目录、Alt + →
      移入前一个目录；拖动至边缘自动滚动，悬停折叠目录可展开，Esc 取消。
    </p>
  </UiPanel>
</template>
