<script setup lang="ts">
import UiScrollArea from './UiScrollArea.vue'
import { inject } from 'vue'
import type { UiSize } from './types'
import { uiFieldContextKey } from './fieldContext'

defineOptions({ inheritAttrs: false })

/** UiField 提供的 label/描述关联（未包在 UiField 里时为 undefined） */
const field = inject(uiFieldContextKey, undefined)

withDefaults(
  defineProps<{
    modelValue?: string
    invalid?: boolean
    resize?: 'none' | 'vertical'
    size?: UiSize
  }>(),
  { modelValue: '', invalid: false, resize: 'vertical', size: 'md' }
)

const emit = defineEmits<{ (event: 'update:modelValue', value: string): void }>()
</script>

<template>
  <!-- attrs（placeholder/rows 等）显式绑到 textarea 本体，不依赖 UiScrollArea as-child 的透传链 -->
  <UiScrollArea as-child axis="vertical" managed>
    <textarea
      v-bind="$attrs"
      :id="($attrs.id as string | undefined) ?? field?.controlId"
      class="field-textarea"
      :class="[
        `ui-textarea-${size}`,
        { 'ui-field-invalid': invalid },
        resize === 'none' ? 'resize-none' : 'resize-y',
      ]"
      :value="modelValue"
      :aria-invalid="invalid || undefined"
      :aria-describedby="
        ($attrs['aria-describedby'] as string | undefined) ?? field?.describedById.value
      "
      @input="emit('update:modelValue', ($event.target as HTMLTextAreaElement).value)"
    />
  </UiScrollArea>
</template>
