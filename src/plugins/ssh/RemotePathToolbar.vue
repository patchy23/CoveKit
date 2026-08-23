<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { UiButton, UiInput } from '@/core/ui'

const props = defineProps<{ currentPath: string; canGoBack?: boolean }>()
const emit = defineEmits<{
  navigate: [path: string]
  back: []
  up: []
  upload: []
  uploadDirectory: []
  download: []
  rename: []
  delete: []
}>()

const editing = ref(false)
const draft = ref(props.currentPath)
const input = ref<{ focus: () => void; select: () => void } | null>(null)
const viewport = ref<HTMLElement | null>(null)
const content = ref<HTMLElement | null>(null)
const overflowing = ref(false)
let resizeObserver: ResizeObserver | null = null

const segments = computed(() => {
  const parts = props.currentPath.split('/').filter(Boolean)
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
  draft.value = props.currentPath
  editing.value = true
  nextTick(() => {
    input.value?.focus()
    input.value?.select()
  })
}
function cancelEdit() {
  draft.value = props.currentPath
  editing.value = false
  nextTick(updateOverflow)
}
function submit() {
  const value = draft.value.trim()
  if (!value) return
  editing.value = false
  emit('navigate', value.startsWith('/') ? value : `/${value}`)
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
  () => props.currentPath,
  (path) => {
    draft.value = path
    editing.value = false
    nextTick(updateOverflow)
  }
)
</script>

<template>
  <div
    class="flex shrink-0 items-center gap-[8px] border-b border-border px-[12px] py-[8px] dark:border-border-dark"
  >
    <UiButton
      variant="ghost"
      size="sm"
      title="返回上一次目录"
      :disabled="!canGoBack"
      @click="emit('back')"
    >
      ← 后退
    </UiButton>
    <UiButton variant="ghost" size="sm" title="上级目录" @click="emit('up')">↑ 上级</UiButton>
    <UiInput
      v-if="editing"
      ref="input"
      v-model="draft"
      size="sm"
      class="flex-1 font-mono"
      spellcheck="false"
      aria-label="输入远程目录路径"
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
        <nav ref="content" class="flex min-w-max shrink-0 items-center" aria-label="远程目录路径">
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
    <UiButton size="sm" @click="emit('upload')">上传文件</UiButton>
    <UiButton size="sm" @click="emit('uploadDirectory')">上传目录</UiButton>
    <UiButton size="sm" @click="emit('download')">下载</UiButton>
    <UiButton size="sm" @click="emit('rename')">重命名</UiButton>
    <UiButton size="sm" class="text-danger-strong dark:text-danger-dark" @click="emit('delete')"
      >删除</UiButton
    >
  </div>
</template>
