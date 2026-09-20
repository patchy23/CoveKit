<script setup lang="ts">
/** 纯显示组件：输入完整快照，暂停不发出停止采集信号；复制交给宿主平台适配。 */
import { nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import UiButton from './UiButton.vue'
import UiIcon from './UiIcon.vue'
import UiScrollArea from './UiScrollArea.vue'
import UiSelect from './UiSelect.vue'
import UiSwitch from './UiSwitch.vue'
import UiToolbar from './UiToolbar.vue'

const props = withDefaults(defineProps<{ content: string; loading?: boolean; error?: string }>(), {
  loading: false,
  error: '',
})
const emit = defineEmits<{ copy: [text: string]; 'limit-change': [lines: number]; refresh: [] }>()
const options = [100, 300, 500, 1000, 2000].map((lines) => ({
  value: String(lines),
  label: `最近 ${lines} 行`,
}))
const limit = ref('300')
const paused = ref(false)
const following = ref(true)
const displayed = ref('')
const output = ref<HTMLElement>()
const selectionText = ref('')
let latest = ''
function tail(content: string) {
  return content
    .replace(/\r?\n$/, '')
    .split('\n')
    .slice(-Number(limit.value))
    .join('\n')
}
async function showLatest() {
  if (paused.value) return
  displayed.value = latest
  await nextTick()
  if (following.value && !paused.value) output.value?.scrollTo({ top: output.value.scrollHeight })
}
watch(
  () => props.content,
  (value) => {
    latest = tail(value)
    void showLatest()
  },
  { immediate: true }
)
watch(paused, () => {
  void showLatest()
})
watch(following, () => {
  void showLatest()
})
function changeLimit(value: string) {
  if (!options.some((option) => option.value === value) || value === limit.value) return
  limit.value = value
  latest = tail(props.content)
  void showLatest()
  emit('limit-change', Number(value))
}
function captureSelection() {
  const selection = window.getSelection()
  selectionText.value = ''
  if (!selection || !selection.rangeCount || !output.value) return
  const range = selection.getRangeAt(0)
  if (output.value.contains(range.startContainer) && output.value.contains(range.endContainer)) {
    selectionText.value = selection.toString()
  }
}
function copySelection() {
  captureSelection()
  if (selectionText.value) emit('copy', selectionText.value)
}
onMounted(() => document.addEventListener('selectionchange', captureSelection))
onUnmounted(() => document.removeEventListener('selectionchange', captureSelection))
</script>

<template>
  <div class="flex min-h-0 min-w-0 flex-1 flex-col">
    <UiToolbar bordered>
      <UiButton
        size="sm"
        :variant="paused ? 'primary' : 'secondary'"
        :aria-pressed="paused"
        @click="paused = !paused"
        ><UiIcon :name="paused ? 'play' : 'pause'" :size="14" />{{
          paused ? '继续显示' : '暂停显示'
        }}</UiButton
      >
      <UiSwitch v-model="following" label="自动跟随" />
      <UiSelect
        :model-value="limit"
        :options="options"
        size="sm"
        title="最多显示行数"
        class="w-[130px]"
        @update:model-value="changeLimit"
      />
      <template #trailing>
        <UiButton
          size="sm"
          variant="ghost"
          :disabled="!selectionText"
          @pointerdown.prevent
          @click="copySelection"
          >复制选中</UiButton
        >
        <UiButton size="sm" variant="ghost" :disabled="!displayed" @click="emit('copy', displayed)"
          >复制全部</UiButton
        >
        <UiButton size="sm" variant="ghost" :loading="loading" @click="emit('refresh')"
          >刷新</UiButton
        >
      </template>
    </UiToolbar>
    <p
      v-if="error"
      role="alert"
      class="select-text shrink-0 px-md py-xs text-body-sm text-danger-strong dark:text-danger-dark"
    >
      {{ error }}
    </p>
    <UiScrollArea as-child axis="both">
      <pre
        ref="output"
        tabindex="0"
        aria-label="日志内容"
        class="min-h-0 min-w-0 flex-1 p-md font-mono text-body-sm whitespace-pre select-text text-primary dark:text-primary-dark"
        @wheel="following = false"
        @pointerdown="following = false"
        @keydown="
          ['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End', ' '].includes($event.key) &&
          (following = false)
        "
        >{{ displayed }}</pre>
    </UiScrollArea>
    <div
      class="flex shrink-0 gap-sm border-t border-border px-md py-xs text-caption text-secondary dark:border-border-dark dark:text-secondary-dark"
    >
      <span>{{
        paused
          ? '已暂停显示 · 后台继续接收，恢复后显示最新日志'
          : loading && !displayed
            ? '正在读取日志…'
            : displayed
              ? '实时显示'
              : '没有日志输出'
      }}</span>
      <span class="ml-auto">{{ displayed ? displayed.split('\n').length : 0 }} 行</span>
    </div>
  </div>
</template>
