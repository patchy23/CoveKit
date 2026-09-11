<script setup lang="ts">
/**
 * ProfileRemarkDialog · 档案备注编辑
 * 备注存在 frp.db（工具侧元数据），**不写进用户的 frpc.toml**，避免污染手写配置。
 */
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { UiButton, UiInput, UiModal } from '@/core/ui'

const props = defineProps<{
  /** 是否可见 */
  open: boolean
  /** 目标档案文件名（标题展示用） */
  fileName: string
  /** 现有备注 */
  initial: string
}>()
const emit = defineEmits<{ submit: [remark: string]; close: [] }>()

const { t } = useI18n()

/** 备注文本（单行输入即可，长备注不常见） */
const remark = ref('')

watch(
  () => [props.open, props.initial] as const,
  ([open]) => {
    if (open) remark.value = props.initial
  },
  { immediate: true }
)

/** 提交备注 */
function onSubmit() {
  emit('submit', remark.value.trim())
}
</script>

<template>
  <UiModal :open="props.open" :title="t('frp.remarkTitle')" size="sm" @close="emit('close')">
    <div class="flex flex-col gap-[8px]">
      <p class="truncate font-mono text-caption text-text-muted dark:text-text-muted-dark" :title="props.fileName">
        {{ props.fileName }}
      </p>
      <UiInput
        v-model="remark"
        :placeholder="t('frp.remarkPlaceholder')"
        @keydown.enter="onSubmit"
      />
      <p class="text-caption text-text-muted dark:text-text-muted-dark">
        {{ t('frp.remarkHint') }}
      </p>
    </div>
    <div class="mt-[14px] flex justify-end gap-[8px]">
      <UiButton variant="secondary" @click="emit('close')">{{ t('frp.dialogCancel') }}</UiButton>
      <UiButton @click="onSubmit">{{ t('frp.remarkSave') }}</UiButton>
    </div>
  </UiModal>
</template>
