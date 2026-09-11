<script setup lang="ts">
/**
 * ProfileNameDialog · 档案名称输入弹窗（新建 / 重命名 / 复制三态复用）
 * 新建时额外选择内置模板；提交前做基本校验（非空、无路径分隔符、自动补 .toml）。
 */
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { UiButton, UiInput, UiModal, UiSelect } from '@/core/ui'
import { FRP_TEMPLATE_IDS, type FrpTemplateId } from '../contracts'

const props = defineProps<{
  /** 是否可见 */
  open: boolean
  /** 弹窗形态 */
  mode: 'create' | 'rename' | 'duplicate'
  /** 初始文件名（重命名/复制时预填） */
  initial?: string
}>()
const emit = defineEmits<{
  /** 提交（create 模式带模板 id） */
  submit: [name: string, template: FrpTemplateId]
  close: []
}>()

const { t } = useI18n()

/** 输入的文件名（对外一律不带扩展名，提交时补 .toml） */
const name = ref('')
/** 新建模板 */
const template = ref<FrpTemplateId>('tcp')
/** 校验错误（空串 = 无错误） */
const error = ref('')

/** 弹窗标题 */
const title = computed(() => {
  if (props.mode === 'rename') return t('frp.dialogRenameTitle')
  if (props.mode === 'duplicate') return t('frp.dialogDuplicateTitle')
  return t('frp.dialogCreateTitle')
})

/** 确认按钮文案 */
const confirmText = computed(() =>
  props.mode === 'rename' ? t('frp.dialogRenameConfirm') : t('frp.dialogCreateConfirm')
)

/** 模板选项 */
const templateOptions = computed(() =>
  FRP_TEMPLATE_IDS.map((id) => ({ value: id, label: t(`frp.template_${id}`) }))
)

/** 去掉 .toml 后缀（用户可能连后缀一起输入） */
function stripExtension(value: string): string {
  return value.trim().replace(/\.toml$/i, '')
}

watch(
  () => [props.open, props.initial] as const,
  ([open]) => {
    if (!open) return
    name.value = stripExtension(props.initial ?? '')
    template.value = 'tcp'
    error.value = ''
  },
  { immediate: true }
)

/** 提交：本地校验通过才向上抛（非法文件名由 Rust 侧二次把关） */
function onSubmit() {
  const trimmed = stripExtension(name.value)
  if (trimmed === '') {
    error.value = t('frp.dialogNameRequired')
    return
  }
  if (/[/\\]/.test(trimmed) || trimmed.includes('..')) {
    error.value = t('frp.dialogNameInvalid')
    return
  }
  emit('submit', `${trimmed}.toml`, template.value)
}

/** 关闭：清掉错误，避免下次打开残留 */
function onClose() {
  error.value = ''
  emit('close')
}
</script>

<template>
  <UiModal :open="props.open" :title="title" size="sm" @close="onClose">
    <div class="flex flex-col gap-[10px]">
      <UiInput
        v-model="name"
        :placeholder="t('frp.dialogNamePlaceholder')"
        @keydown.enter="onSubmit"
      />
      <UiSelect v-if="props.mode === 'create'" v-model="template" :options="templateOptions" />
      <p v-if="error !== ''" class="text-body-sm text-danger-strong dark:text-danger-dark">
        {{ error }}
      </p>
      <p class="text-caption text-text-muted dark:text-text-muted-dark">
        {{ t('frp.dialogNameHint') }}
      </p>
    </div>
    <div class="mt-[14px] flex justify-end gap-[8px]">
      <UiButton variant="secondary" @click="onClose">{{ t('frp.dialogCancel') }}</UiButton>
      <UiButton @click="onSubmit">{{ confirmText }}</UiButton>
    </div>
  </UiModal>
</template>
