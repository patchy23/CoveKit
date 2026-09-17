<script setup lang="ts">
/**
 * PathBreadcrumbs · 可编辑面包屑路径条（SSH 文件双栏共用：远程 / 分隔符、本地 \ 分隔符）
 * 默认态：分段面包屑，点段跳转，点空白处进入编辑；超长时右对齐显示尾部 + 左侧 … 提示。
 * 编辑态：完整路径输入框，Enter 提交 / Esc 或失焦取消。
 */
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { UiButton, UiInput } from '@/core/ui'
import { normalizePathInput } from './pathInput'

const props = withDefaults(
  defineProps<{
    /** 当前完整路径 */
    path: string
    /** 路径分隔符（远程 '/'、本地 '\\'） */
    separator?: string
  }>(),
  { separator: '/' }
)

const emit = defineEmits<{
  /** 提交新路径（已按分隔符规范化） */
  navigate: [path: string]
}>()

const editing = ref(false)
const draft = ref(props.path)
const input = ref<{ focus: () => void; select: () => void } | null>(null)
const viewport = ref<HTMLElement | null>(null)
const content = ref<HTMLElement | null>(null)
const overflowing = ref(false)
let resizeObserver: ResizeObserver | null = null

/** 路径分段（首段为根：'/' 或盘符如 'C:'） */
const segments = computed(() => {
  const sep = props.separator
  const parts = props.path.split(sep === '\\' ? /[\\/]+/ : /\/+/).filter(Boolean)
  if (sep === '\\') {
    // Windows：首段是盘符（C:），面包屑根 = 盘符本身
    return parts.map((part, index) => ({
      label: part,
      path: parts.slice(0, index + 1).join('\\') + (index === 0 ? '\\' : ''),
    }))
  }
  return [
    { label: '/', path: '/' },
    ...parts.map((part, index) => ({
      label: part,
      path: `/${parts.slice(0, index + 1).join('/')}`,
    })),
  ]
})

function updateOverflow() {
  overflowing.value = Boolean(
    viewport.value && content.value && content.value.scrollWidth > viewport.value.clientWidth + 1
  )
}
function beginEdit() {
  draft.value = props.path
  editing.value = true
  nextTick(() => {
    input.value?.focus()
    input.value?.select()
  })
}
function cancelEdit() {
  draft.value = props.path
  editing.value = false
  nextTick(updateOverflow)
}
function submit() {
  const value = draft.value.trim()
  if (!value) return
  editing.value = false
  const normalized = normalizePathInput(value, props.separator)
  emit('navigate', normalized)
}

onMounted(() => {
  resizeObserver = new ResizeObserver(updateOverflow)
  if (viewport.value) resizeObserver.observe(viewport.value)
  nextTick(updateOverflow)
})
onBeforeUnmount(() => resizeObserver?.disconnect())
watch(viewport, (element, previous) => {
  if (previous) resizeObserver?.unobserve(previous)
  if (element) resizeObserver?.observe(element)
  nextTick(updateOverflow)
})
watch(
  () => props.path,
  (path) => {
    draft.value = path
    editing.value = false
    nextTick(updateOverflow)
  }
)
</script>

<template>
  <UiInput
    v-if="editing"
    ref="input"
    v-model="draft"
    size="sm"
    class="flex-1 font-mono"
    spellcheck="false"
    aria-label="输入完整路径"
    @blur="cancelEdit"
    @keyup.enter="submit"
    @keyup.esc="cancelEdit"
  />
  <div
    v-else
    class="flex h-[30px] min-w-0 flex-1 cursor-text items-center overflow-hidden rounded-md border border-border-strong bg-surface-muted pl-[4px] dark:border-border-strong-dark dark:bg-surface-muted-dark"
    title="点击空白处输入完整路径"
    @click="beginEdit"
  >
    <div
      ref="viewport"
      class="relative flex min-w-0 flex-1 overflow-hidden"
      :class="overflowing ? 'justify-end' : 'justify-start'"
    >
      <span
        v-if="overflowing"
        class="absolute inset-y-0 left-0 z-10 flex items-center bg-surface-muted px-[6px] text-body-sm text-text-muted dark:bg-surface-muted-dark dark:text-text-muted-dark"
        >…</span
      >
      <nav ref="content" class="flex min-w-max shrink-0 items-center" aria-label="路径">
        <template v-for="(segment, index) in segments" :key="segment.path">
          <span
            v-if="index > 0"
            class="px-[1px] text-caption text-text-muted dark:text-text-muted-dark"
            >›</span
          >
          <UiButton
            variant="ghost"
            size="xs"
            class="font-mono"
            :title="`进入 ${segment.path}`"
            @click.stop="emit('navigate', segment.path)"
          >
            {{ segment.label }}
          </UiButton>
        </template>
      </nav>
    </div>
    <span class="h-full w-[36px] shrink-0" aria-hidden="true" />
  </div>
</template>
