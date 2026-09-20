<script setup lang="ts">
/**
 * 编辑器查找替换面板（中文界面，浮于编辑器右上角）
 *
 * 职责边界：只收集条件并发出意图，不做匹配计算——匹配总数与当前序号由宿主传入，
 * 宿主用 `search.ts` 的同一套语义执行跳转与高亮，保证面板显示与实际行为一致。
 */
import { computed, ref, watch } from 'vue'
import UiButton from '../UiButton.vue'
import UiInput from '../UiInput.vue'
import type { SearchOptions } from './search'

const props = withDefaults(
  defineProps<{
    /** 匹配总数 */
    total?: number
    /** 当前匹配序号（1 起始；0 表示未定位） */
    current?: number
    /** 正则无效时的中文提示（面板内红字，不弹 toast） */
    error?: string
    /** 初始是否展开替换行 */
    replaceMode?: boolean
    /** 只读编辑器隐藏替换入口 */
    readonly?: boolean
  }>(),
  { total: 0, current: 0, error: undefined, replaceMode: false, readonly: false }
)

const emit = defineEmits<{
  (event: 'search', payload: { query: string; replacement: string; options: SearchOptions }): void
  (event: 'next'): void
  (event: 'previous'): void
  (event: 'replace'): void
  (event: 'replace-all'): void
  (event: 'close'): void
}>()

const query = ref('')
const replacement = ref('')
const caseSensitive = ref(false)
const regexp = ref(false)
const wholeWord = ref(false)
const showReplace = ref(props.replaceMode)

/** 查找输入框引用（宿主可通过 expose 的 focus() 聚焦） */
const queryInput = ref<{ focus?: () => void; select?: () => void } | null>(null)

/** 计数文案：无查询条件时留空，出错时显示 0/0 */
const counter = computed(() => {
  if (!query.value) return ''
  if (props.error) return '0/0'
  return `${props.current}/${props.total}`
})

/** 是否可执行替换：只读、无查询或无匹配时禁用 */
const replaceDisabled = computed(() => props.readonly || !query.value || props.total === 0)

watch([query, replacement, caseSensitive, regexp, wholeWord], () => {
  emit('search', {
    query: query.value,
    replacement: replacement.value,
    options: {
      caseSensitive: caseSensitive.value,
      regexp: regexp.value,
      wholeWord: wholeWord.value,
    },
  })
})

/** 聚焦并选中查找框内容（宿主打开面板时调用） */
function focus() {
  queryInput.value?.focus?.()
  queryInput.value?.select?.()
}

/** 切换替换行（面板内「替换」入口使用） */
function toggleReplace() {
  showReplace.value = !showReplace.value
}

defineExpose({ focus, toggleReplace })
</script>

<template>
  <div
    class="flex w-[460px] flex-col gap-2 rounded-md border border-border bg-surface p-2 shadow-lg dark:border-border-dark dark:bg-surface-dark"
    @keydown.esc.stop="emit('close')"
  >
    <div class="flex items-center gap-2">
      <div class="w-[168px]">
        <UiInput
          ref="queryInput"
          v-model="query"
          size="sm"
          placeholder="查找"
          :invalid="Boolean(error)"
          @keydown.enter.prevent="emit('next')"
        />
      </div>
      <span class="w-[46px] shrink-0 text-caption text-text-muted dark:text-text-muted-dark">
        {{ counter }}
      </span>
      <UiButton size="sm" variant="ghost" title="上一个匹配" @click="emit('previous')"
        >上一个</UiButton
      >
      <UiButton size="sm" variant="ghost" title="下一个匹配" @click="emit('next')">下一个</UiButton>
      <div class="mx-0.5 h-4 w-px shrink-0 bg-border dark:bg-border-dark" />
      <UiButton
        size="sm"
        :variant="caseSensitive ? 'primary' : 'ghost'"
        title="区分大小写"
        @click="caseSensitive = !caseSensitive"
      >
        Aa
      </UiButton>
      <UiButton
        size="sm"
        :variant="regexp ? 'primary' : 'ghost'"
        title="正则表达式"
        @click="regexp = !regexp"
      >
        .*
      </UiButton>
      <UiButton
        size="sm"
        :variant="wholeWord ? 'primary' : 'ghost'"
        title="全词匹配"
        @click="wholeWord = !wholeWord"
      >
        全词
      </UiButton>
      <UiButton size="sm" variant="ghost" title="关闭（Esc）" @click="emit('close')">关闭</UiButton>
    </div>

    <div v-if="showReplace && !readonly" class="flex items-center gap-2">
      <div class="w-[168px]">
        <UiInput v-model="replacement" size="sm" placeholder="替换为" />
      </div>
      <UiButton size="sm" variant="ghost" :disabled="replaceDisabled" @click="emit('replace')"
        >替换</UiButton
      >
      <UiButton size="sm" variant="ghost" :disabled="replaceDisabled" @click="emit('replace-all')">
        全部替换
      </UiButton>
    </div>

    <p v-if="error" class="select-text text-caption text-danger-strong dark:text-danger-dark">
      {{ error }}
    </p>
  </div>
</template>
