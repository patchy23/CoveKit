<script setup lang="ts">
/** 树形单选浮层；复用公共树的焦点、键盘与展开交互，不绑定业务状态。 */
import { computed, nextTick, ref, watch } from 'vue'
import { PopoverRoot, PopoverTrigger, PopoverPortal, PopoverContent } from 'reka-ui'
import UiButton from './UiButton.vue'
import UiIcon from './UiIcon.vue'
import UiTree from './UiTree.vue'
import UiInput from './UiInput.vue'
import { treeSelectRows, type TreeSelectOption } from './treeSelect'
import type { UiSize } from './types'
import { UI_FLOATING_PANEL_CLASS } from './utils'
const props = withDefaults(
  defineProps<{
    modelValue: string
    options: TreeSelectOption[]
    disabled?: boolean
    size?: UiSize
    label?: string
    placeholder?: string
  }>(),
  { size: 'md', label: '选择分组', placeholder: '请选择' }
)
const emit = defineEmits<{ 'update:modelValue': [value: string] }>()
const open = ref(false),
  query = ref(''),
  expanded = ref(new Set<string>())
const panel = ref<HTMLElement | null>(null)
const tree = computed(() => treeSelectRows(props.options, expanded.value, query.value))
watch(open, (value) => {
  if (!value) return
  query.value = ''
  // 打开时展开当前选中项的祖先，保留用户此前展开的其他分支。
  function reveal(nodes: TreeSelectOption[], parents: string[]): boolean {
    for (const node of nodes) {
      if (node.value === props.modelValue) {
        parents.forEach((id) => expanded.value.add(id))
        return true
      }
      if (reveal(node.children ?? [], [...parents, node.value])) return true
    }
    return false
  }
  reveal(props.options, [])
})
watch(
  () => props.disabled,
  (value) => {
    if (value) open.value = false
  }
)
function toggle(id: string) {
  if (expanded.value.has(id)) expanded.value.delete(id)
  else expanded.value.add(id)
}
function select(value: string) {
  if (props.disabled || tree.value.rows.find((row) => row.id === value)?.disabled) return
  emit('update:modelValue', value)
  open.value = false
}
async function autofocus(event: Event) {
  event.preventDefault()
  await nextTick()
  panel.value?.querySelector<HTMLInputElement>('input')?.focus()
}
function focusTree(event: KeyboardEvent) {
  event.preventDefault()
  panel.value?.querySelector<HTMLElement>('[role="treeitem"][tabindex="0"]')?.focus()
}
</script>
<template>
  <PopoverRoot v-model:open="open">
    <PopoverTrigger as-child>
      <UiButton
        :size="size"
        :disabled="disabled"
        :aria-label="label"
        block
        class="!justify-between"
      >
        <span class="min-w-0 truncate">{{ tree.labels.get(modelValue) ?? placeholder }}</span>
        <UiIcon
          name="chevron-down"
          :size="14"
          class="shrink-0 text-secondary dark:text-secondary-dark"
        />
      </UiButton>
    </PopoverTrigger>
    <PopoverPortal>
      <PopoverContent
        align="start"
        :side-offset="4"
        :class="[
          UI_FLOATING_PANEL_CLASS,
          'pointer-events-auto w-[var(--reka-popover-trigger-width)] min-w-[200px] max-w-[calc(100vw-24px)]',
        ]"
        @open-auto-focus="autofocus"
        @escape-key-down.stop
      >
        <div
          ref="panel"
          class="flex max-h-[min(300px,var(--reka-popover-content-available-height))] flex-col p-[4px]"
        >
          <UiInput
            v-model="query"
            size="sm"
            class="shrink-0"
            placeholder="搜索…"
            :aria-label="`搜索${label}`"
            @keydown.down="focusTree"
          />
          <UiTree
            :items="tree.rows"
            :model-value="modelValue"
            :label="label"
            :row-height="28"
            empty-text="无匹配项"
            @toggle="toggle($event.id)"
            @select="select($event.id)"
          />
        </div>
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>
