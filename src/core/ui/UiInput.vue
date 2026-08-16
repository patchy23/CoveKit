<script setup lang="ts">
/**
 * UiInput · 文本输入（field-input 样式，高度基线随 ui-control-* 档位）
 * type="password" 时自带输入框内眼睛切换明文（不依赖 WebView2 原生 reveal，行为确定）。
 */
import { computed, ref } from 'vue'
import UiIcon from './UiIcon.vue'
import type { UiSize } from './types'

defineOptions({ inheritAttrs: false })

const props = withDefaults(
  defineProps<{
    modelValue?: string | number
    type?: string
    invalid?: boolean
    size?: UiSize
    modelModifiers?: { number?: boolean; trim?: boolean }
  }>(),
  { modelValue: '', type: 'text', invalid: false, size: 'md', modelModifiers: () => ({}) }
)

const emit = defineEmits<{ (event: 'update:modelValue', value: string | number): void }>()
const input = ref<HTMLInputElement | null>(null)

defineExpose({
  focus: () => input.value?.focus(),
  select: () => input.value?.select(),
})

/** 密码明文开关（仅 type=password 生效） */
const reveal = ref(false)
/** 实际渲染的 input type */
const realType = computed(() => (props.type === 'password' && reveal.value ? 'text' : props.type))

function onInput(event: Event) {
  let value: string | number = (event.target as HTMLInputElement).value
  if (props.modelModifiers.trim) value = value.trim()
  if (props.modelModifiers.number && value !== '') value = Number(value)
  emit('update:modelValue', value)
}
</script>

<template>
  <!-- 密码：带眼睛的包裹结构（外部传入的 class/placeholder 等透传到 input 本体） -->
  <div v-if="type === 'password'" class="relative w-full">
    <input
      ref="input"
      v-bind="$attrs"
      :type="realType"
      class="field-input pr-[30px]"
      :class="[`ui-control-${size}`, { 'ui-field-invalid': invalid }]"
      :value="modelValue"
      :aria-invalid="invalid || undefined"
      @input="onInput"
    />
    <button
      type="button"
      class="absolute right-[8px] top-1/2 grid h-[20px] w-[20px] -translate-y-1/2 place-items-center rounded-[4px] text-text-muted transition-colors hover:text-primary dark:text-text-muted-dark dark:hover:text-primary-dark"
      :title="reveal ? '隐藏' : '显示'"
      tabindex="-1"
      @click="reveal = !reveal"
    >
      <UiIcon :name="reveal ? 'eye-off' : 'eye'" :size="13" />
    </button>
  </div>
  <input
    v-else
    ref="input"
    v-bind="$attrs"
    :type="realType"
    class="field-input"
    :class="[`ui-control-${size}`, { 'ui-field-invalid': invalid }]"
    :value="modelValue"
    :aria-invalid="invalid || undefined"
    @input="onInput"
  />
</template>
