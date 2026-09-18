<script setup lang="ts">
/** 表格主行与跨列详情；受控展开，默认收起即卸载，保留模式也按首次展开懒挂载。 */
import { ref, useId, watch } from 'vue'
import UiIcon from './UiIcon.vue'
import UiIconButton from './UiIconButton.vue'
import UiTableCell from './UiTableCell.vue'

const props = withDefaults(
  defineProps<{
    expanded: boolean
    columns: number
    label: string
    disabled?: boolean
    keepMounted?: boolean
  }>(),
  { disabled: false, keepMounted: false }
)
const emit = defineEmits<{ 'update:expanded': [value: boolean] }>()
const detailsId = useId()
const visited = ref(props.expanded)
watch(
  () => props.expanded,
  (value) => {
    if (value) visited.value = true
  }
)
function toggle() {
  if (!props.disabled) emit('update:expanded', !props.expanded)
}
</script>

<template>
  <tr :data-expanded="expanded">
    <UiTableCell>
      <div class="flex items-center gap-xs">
        <UiIconButton
          size="xs"
          :label="`${expanded ? '收起' : '展开'}${label}`"
          :disabled="disabled"
          :aria-expanded="expanded"
          :aria-controls="detailsId"
          @click="toggle"
        >
          <UiIcon name="chevron-right" :size="14" :class="{ 'rotate-90': expanded }" />
        </UiIconButton>
        <slot name="label">{{ label }}</slot>
      </div>
    </UiTableCell>
    <slot :toggle="toggle" :expanded="expanded" :details-id="detailsId" />
  </tr>
  <tr v-if="expanded || (keepMounted && visited)" v-show="expanded" data-table-detail>
    <UiTableCell :colspan="columns" class="!p-0">
      <div
        :id="detailsId"
        role="region"
        :aria-label="label"
        class="border-l-2 border-border-strong bg-surface-muted px-md py-sm dark:border-border-strong-dark dark:bg-surface-muted-dark"
      >
        <slot name="details" />
      </div>
    </UiTableCell>
  </tr>
</template>
