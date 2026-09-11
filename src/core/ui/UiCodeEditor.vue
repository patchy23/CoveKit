<script setup lang="ts">
/**
 * 通用代码编辑器（编辑器组件的唯一对外入口）
 *
 * - `readonly` 时即查看器（取代原 CodeViewer），语义与视觉保持一致；
 * - 明暗、语言、只读、缩进、换行全部走 Compartment 热重配置，切换不重建实例（不丢撤销历史）；
 * - `mode="minimal"` 为轻量档：替代原「行号 + textarea」输入输出区（无折叠 / 括号闭合 / 列选择）。
 */
import { onMounted, ref } from 'vue'
import { useCodeEditor } from './editor/useCodeEditor'
import type { EditorCursorInfo } from './editor/types'
import type { EditorMode } from './editor/extensions'

const props = withDefaults(
  defineProps<{
    /** 编辑内容（v-model） */
    modelValue?: string
    /** 语言 id；默认 'auto' 表示按 filename 识别 */
    language?: string
    /** 文件名或路径：language='auto' 时用于识别，亦用于状态栏展示 */
    filename?: string
    /** 只读（查看器形态，快捷键与折叠仍可用） */
    readonly?: boolean
    /** 档位：full 完整能力，minimal 轻量输入区 */
    mode?: EditorMode
    /** 是否显示行号 */
    lineNumbers?: boolean
    /** 是否显示折叠 gutter（仅 full 档生效） */
    foldGutter?: boolean
    /** 长行是否换行 */
    lineWrapping?: boolean
    /** 缩进宽度（空格数） */
    tabSize?: number
    /** 空文档占位提示 */
    placeholder?: string
    /** 容器高度（CSS 长度值） */
    height?: string
  }>(),
  {
    modelValue: '',
    language: 'auto',
    filename: undefined,
    readonly: false,
    mode: 'full',
    lineNumbers: true,
    foldGutter: true,
    lineWrapping: false,
    tabSize: 2,
    placeholder: undefined,
    height: '100%',
  }
)

const emit = defineEmits<{
  (event: 'update:modelValue', value: string): void
  (event: 'change', value: string): void
  (event: 'cursor', info: EditorCursorInfo): void
}>()

/** 编辑器挂载容器 */
const hostRef = ref<HTMLDivElement | null>(null)

const editor = useCodeEditor({
  modelValue: () => props.modelValue ?? '',
  filename: () => props.filename,
  language: () => props.language,
  readonly: () => props.readonly,
  mode: () => props.mode,
  lineNumbers: () => props.lineNumbers,
  foldGutter: () => props.foldGutter,
  lineWrapping: () => props.lineWrapping,
  tabSize: () => props.tabSize,
  placeholder: () => props.placeholder,
  onChange: (value) => {
    emit('update:modelValue', value)
    emit('change', value)
  },
  onCursor: (info) => emit('cursor', info),
})

onMounted(() => {
  editor.host.value = hostRef.value
  editor.mount()
})

defineExpose({
  /** 聚焦 */
  focus: () => editor.focus(),
  /** 失焦 */
  blur: () => editor.blur(),
  /** 读取当前内容 */
  getValue: () => editor.getValue(),
  /** 写入内容（默认不进撤销历史） */
  setValue: (value: string, options?: { addToHistory?: boolean }) =>
    editor.setValue(value, options),
  /** 跳转到指定行（1 起始） */
  goToLine: (line: number) => editor.goToLine(line),
  /** 读取选中文本 */
  getSelection: () => editor.getSelection(),
  /** 在光标处插入文本 */
  insert: (text: string) => editor.insert(text),
  /** 撤销 */
  undo: () => editor.undo(),
  /** 重做 */
  redo: () => editor.redo(),
  /** 当前语言中文标签（自动识别或显式指定） */
  getLanguageLabel: () => editor.languageInfo.value.label,
})
</script>

<template>
  <div
    class="ui-code-editor overflow-hidden rounded-lg border border-border bg-surface dark:border-border-dark dark:bg-surface-dark"
    :style="{ height }"
  >
    <div ref="hostRef" class="h-full w-full" />
  </div>
</template>
