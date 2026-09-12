<script setup lang="ts">
/**
 * HostKeyDialog · 主机密钥人工确认弹窗
 * kind=unknown：首次连接，展示算法 + SHA256 指纹，可选「仅本次信任 / 保存并连接 / 取消」；
 * kind=mismatch：指纹变更（可能是重装也可能是中间人攻击），默认取消，替换需二次输入确认。
 */
import { computed, ref } from 'vue'
import type { HostKeyVerifyRequest } from '../contracts'
import { UiButton, UiInput, UiModal } from '@/core/ui'

const props = defineProps<{
  request: HostKeyVerifyRequest | null
}>()

const emit = defineEmits<{
  (e: 'respond', decision: 'trustOnce' | 'trustSave' | 'cancel' | 'replace'): void
}>()

const isMismatch = computed(() => props.request?.kind === 'mismatch')
/** 替换指纹的二次确认输入（须完整输入 C O N F I R M？——用主机名更直观） */
const replaceConfirmText = ref('')

function canReplace() {
  return (
    Boolean(props.request) && replaceConfirmText.value.trim() === (props.request?.host ?? '___')
  )
}

function respond(decision: 'trustOnce' | 'trustSave' | 'cancel' | 'replace') {
  replaceConfirmText.value = ''
  emit('respond', decision)
}
</script>

<template>
  <UiModal
    :open="Boolean(request)"
    :title="isMismatch ? '安全警告：主机密钥已变更' : '确认服务器主机密钥'"
    width="min(460px, 92vw)"
    @close="respond('cancel')"
  >
    <div class="space-y-[10px]">
      <p class="text-body-sm text-secondary dark:text-secondary-dark">
        服务器 <span class="font-mono">{{ request?.host }}</span> :{{ request?.port }} 的{{
          isMismatch ? '主机密钥与已保存指纹不一致' : '主机密钥首次出现'
        }}。
        {{
          isMismatch ? '这可能意味着服务器重装，也可能存在中间人攻击风险。' : '请核对指纹后再信任。'
        }}
      </p>

      <div class="space-y-[4px] rounded-md bg-surface-muted p-[10px] dark:bg-surface-muted-dark">
        <div class="flex items-center justify-between gap-[10px]">
          <span class="text-caption text-text-muted dark:text-text-muted-dark">算法</span>
          <span class="font-mono text-caption">{{ request?.algorithm }}</span>
        </div>
        <div class="flex items-center justify-between gap-[10px]">
          <span class="shrink-0 text-caption text-text-muted dark:text-text-muted-dark">
            {{ isMismatch ? '新指纹' : '指纹（SHA256）' }}
          </span>
          <span class="min-w-0 break-all text-right font-mono text-caption">{{
            request?.fingerprint
          }}</span>
        </div>
        <template v-if="isMismatch">
          <div class="flex items-center justify-between gap-[10px]">
            <span class="shrink-0 text-caption text-text-muted dark:text-text-muted-dark">
              已保存指纹
            </span>
            <span
              class="min-w-0 break-all text-right font-mono text-caption text-danger-strong dark:text-danger-dark"
            >
              {{ request?.savedFingerprints[0] }}
            </span>
          </div>
        </template>
      </div>

      <p v-if="isMismatch" class="text-caption text-danger-strong dark:text-danger-dark">
        默认取消连接。如确认是服务器合法变更（如重装系统），可输入主机名「{{
          request?.host
        }}」解锁替换。
      </p>
      <UiInput
        v-if="isMismatch"
        v-model="replaceConfirmText"
        class="font-mono"
        :placeholder="request?.host"
      />
      <p v-if="!isMismatch" class="text-caption text-text-muted dark:text-text-muted-dark">
        选择「保存并连接」后，同一服务器后续连接将自动校验该指纹。
      </p>
    </div>

    <template #footer>
      <UiButton variant="ghost" @click="respond('cancel')">取消</UiButton>
      <template v-if="isMismatch">
        <UiButton variant="ghost" @click="respond('trustOnce')">仅本次信任新指纹</UiButton>
        <UiButton
          variant="danger"
          :disabled="!canReplace()"
          title="替换需要输入主机名确认"
          @click="canReplace() && respond('replace')"
        >
          替换指纹并连接
        </UiButton>
      </template>
      <template v-else>
        <UiButton variant="ghost" @click="respond('trustOnce')">仅本次信任</UiButton>
        <UiButton variant="primary" @click="respond('trustSave')">保存并连接</UiButton>
      </template>
    </template>
  </UiModal>
</template>
