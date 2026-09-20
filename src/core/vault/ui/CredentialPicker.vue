<script setup lang="ts">
/**
 * CredentialPicker · 凭证选择器（core/vault 复合组件，经 @/core/vault 使用）
 * 按 kind 过滤列出凭证库条目（显示名称 + 掩码摘要），末尾「+ 新建凭证」内嵌
 * CredentialForm，保存成功后自动选中。空串值 = 未选择（工具可据此回退手填）。
 */
import { computed, onMounted, ref, watch } from 'vue'
import { UiSelect } from '@/core/ui'
import type { CredentialKind, CredentialSummary } from '@/core/ipc/contracts'
import { ipc } from '@/core/ipc/ipc'
import { KIND_LABEL } from '../useVault'
import CredentialForm from './CredentialForm.vue'

const props = withDefaults(
  defineProps<{
    /** 选中的凭证 id（'' = 未选择） */
    modelValue: string
    /** 限定类型（不传 = 全部类型） */
    kind?: CredentialKind
    placeholder?: string
    disabled?: boolean
    size?: 'xs' | 'sm' | 'md' | 'lg'
  }>(),
  { kind: undefined, placeholder: '选择凭证（可选）', disabled: false, size: 'sm' }
)

const emit = defineEmits<{ (event: 'update:modelValue', value: string): void }>()

/** 新建选项的哨兵值（选中它 = 打开新建表单） */
const CREATE_VALUE = '__create__'
/**
 * 「不使用凭证」的哨兵值（选中它 = 清空选择）。
 * reka 的 SelectItem 在 value 为空串时会直接抛错——空串被它保留表示「清空选择」，
 * 所以空值选项必须用非空占位值，在选中时换算回空串。
 */
const NONE_VALUE = '__none__'

const list = ref<CredentialSummary[]>([])
const formOpen = ref(false)
const loaded = ref(false)
const loadFailed = ref(false)

/** 下拉选项：凭证条目 + 末尾新建入口 */
const options = computed(() => {
  const items = list.value.map((c) => ({
    value: c.id,
    label: `${c.name}（${c.masked}）`,
  }))
  items.push({ value: NONE_VALUE, label: '不使用凭证' })
  items.push({ value: CREATE_VALUE, label: '+ 新建凭证' })
  return items
})

/** 已保存的引用不在列表中：通常是凭证被删除或导入覆盖。 */
const selectionError = computed(() => {
  if (!loaded.value || !props.modelValue) return ''
  if (loadFailed.value) return '凭证库暂不可用；可稍后重试或清空选择并改用手工凭据。'
  return list.value.some((c) => c.id === props.modelValue)
    ? ''
    : '所选凭证已删除或不可用，请重新选择。'
})

async function reload() {
  loadFailed.value = false
  try {
    const all = await ipc.vaultList()
    list.value = props.kind ? all.filter((c) => c.kind === props.kind) : all
  } catch {
    // 凭证库不可用时降级为空列表（工具仍可手填）
    list.value = []
    loadFailed.value = true
  } finally {
    loaded.value = true
  }
}

onMounted(reload)
watch(() => props.kind, reload)

function onSelect(value: string) {
  if (value === CREATE_VALUE) {
    formOpen.value = true
    return
  }
  emit('update:modelValue', value === NONE_VALUE ? '' : value)
}

/** 新建保存成功：刷新列表并自动选中 */
function onSaved(summary: CredentialSummary) {
  void reload()
  emit('update:modelValue', summary.id)
}
</script>

<template>
  <UiSelect
    :model-value="modelValue === '' ? NONE_VALUE : modelValue"
    :options="options"
    :placeholder="placeholder"
    :disabled="disabled"
    :size="size"
    :title="kind ? `仅显示「${KIND_LABEL[kind]}」类型凭证` : ''"
    @update:model-value="onSelect"
  />
  <CredentialForm
    :open="formOpen"
    :initial-kind="kind ?? 'password'"
    @close="formOpen = false"
    @saved="onSaved"
  />
  <p
    v-if="selectionError"
    class="select-text text-caption text-danger-strong dark:text-danger-dark"
  >
    {{ selectionError }}
  </p>
</template>
