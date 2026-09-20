<script setup lang="ts">
/**
 * SSH 终端搜索条（浮层，任务书批 1 §1.4）
 * 职责边界：只收集条件、发意图，不做匹配计算；计数与实际跳转都由 useTerminalSearch 驱动，
 * 本组件按同一套计数语义渲染出来（交互与视觉对齐编辑器查找面板 EditorSearchBar）。
 * 浮在终端右上角，不占终端高度：宿主终端容器需 position: relative。
 * 所有可点元素走 @/core/ui 公共组件（契约测试 nativeControls 会拒绝工具层出现原生表单控件）。
 */
import { computed, nextTick, onMounted, ref } from 'vue'
import { UiButton, UiIcon, UiIconButton, UiInput } from '@/core/ui'
import { formatResultCount, shouldSearch } from './useTerminalSearch'

const props = withDefaults(
  defineProps<{
    query?: string
    /** 当前匹配序号（0 起始；-1 = 未定位），透传 useTerminalSearch.resultIndex */
    index?: number
    /** 命中总数，透传 useTerminalSearch.resultCount */
    total?: number
    /** Aa 区分大小写 */
    caseSensitive?: boolean
    /** .* 正则模式 */
    regex?: boolean
    /** 正则非法等异常的中文提示（红字；不弹 toast、不进 console） */
    error?: string
  }>(),
  { query: '', index: 0, total: 0, caseSensitive: false, regex: false, error: '' }
)

const emit = defineEmits<{
  (event: 'update:query', value: string): void
  (event: 'update:caseSensitive', value: boolean): void
  (event: 'update:regex', value: boolean): void
  (event: 'next'): void
  (event: 'previous'): void
  (event: 'close'): void
}>()

/** 受控输入：真值在 useTerminalSearch，面板不另存一份（避免两处状态打架） */
const query = computed({
  get: () => props.query,
  set: (value: string) => emit('update:query', value),
})

/** 计数：空查询留空；无匹配 0/0；与 useTerminalSearch 共用 formatResultCount */
const counter = computed(() => formatResultCount(props.index, props.total, props.query))

/** 无匹配空态：仅一行灰字，且不覆盖错误态（错误优先显示红字） */
const noMatch = computed(() => shouldSearch(props.query) && !props.error && props.total === 0)

const queryInput = ref<{ focus?: () => void; select?: () => void } | null>(null)

/** Enter 下一个 / Shift+Enter 上一个 */
function onEnter(event: KeyboardEvent) {
  if (event.shiftKey) emit('previous')
  else emit('next')
}

// 面板由 v-if 挂在可见时（Ctrl+F 呼出），挂载即聚焦并全选，省去宿主再调一次 focus
onMounted(async () => {
  await nextTick()
  queryInput.value?.focus?.()
  queryInput.value?.select?.()
})
</script>

<template>
  <div
    class="absolute right-[12px] top-[8px] z-10 flex flex-col gap-[4px] rounded-md border border-border bg-surface p-[8px] shadow-lg dark:border-border-dark dark:bg-surface-dark"
    @keydown.esc.stop="emit('close')"
  >
    <div class="flex items-center gap-[6px]">
      <div class="w-[140px]">
        <UiInput
          ref="queryInput"
          v-model="query"
          size="sm"
          placeholder="在终端中查找"
          :invalid="Boolean(error)"
          @keydown.enter.prevent="onEnter"
        />
      </div>
      <span
        class="w-[46px] shrink-0 font-mono text-caption text-text-muted dark:text-text-muted-dark"
      >
        {{ counter }}
      </span>
      <UiIconButton label="上一个匹配（Shift+Enter）" size="sm" @click="emit('previous')">
        <UiIcon name="chevron-up" :size="13" />
      </UiIconButton>
      <UiIconButton label="下一个匹配（Enter）" size="sm" @click="emit('next')">
        <UiIcon name="chevron-down" :size="13" />
      </UiIconButton>
      <div class="mx-[2px] h-[16px] w-px shrink-0 bg-border dark:bg-border-dark" />
      <UiButton
        size="sm"
        :variant="caseSensitive ? 'primary' : 'ghost'"
        title="区分大小写"
        @click="emit('update:caseSensitive', !caseSensitive)"
      >
        Aa
      </UiButton>
      <UiButton
        size="sm"
        :variant="regex ? 'primary' : 'ghost'"
        title="正则表达式"
        @click="emit('update:regex', !regex)"
      >
        .*
      </UiButton>
      <UiIconButton label="关闭（Esc）" size="sm" @click="emit('close')">
        <UiIcon name="x" :size="13" />
      </UiIconButton>
    </div>
    <p v-if="error" class="select-text text-caption text-danger-strong dark:text-danger-dark">
      {{ error }}
    </p>
    <p v-else-if="noMatch" class="text-caption text-text-muted dark:text-text-muted-dark">无匹配</p>
  </div>
</template>
