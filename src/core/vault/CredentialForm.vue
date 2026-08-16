<script setup lang="ts">
/**
 * CredentialForm · 凭证编辑表单弹窗（框架级，core/vault）
 * 按 kind 动态渲染字段（formFieldsFor），秘密字段用 type=password（明文切换走输入框自带眼睛）；
 * custom 类型用键值条目编辑器。被凭证管理页与 CredentialPicker（+ 新建凭证）共用。
 */
import { reactive, ref, watch } from 'vue'
import type { Credential, CredentialKind, CredentialSummary } from '@/core/ipc/contracts'
import { ipc } from '@/core/ipc/ipc'
import {
  emptyFormState,
  formFieldsFor,
  formStateFromCredential,
  payloadFromFormState,
  validateFormState,
  KIND_LABEL,
  CREDENTIAL_KINDS,
  type CredentialFormState,
} from '@/core/vault/useVault'
import {
  UiAlert,
  UiButton,
  UiCheckbox,
  UiIcon,
  UiIconButton,
  UiInput,
  UiModal,
  UiSelect,
  UiTextarea,
} from '@/core/ui'
import { useUiStore } from '@/stores/ui'

const props = withDefaults(
  defineProps<{
    /** 弹窗开关 */
    open: boolean
    /** 编辑目标（reveal 后的明文；null = 新建） */
    credential?: Credential | null
    /** 新建时的初始类型 */
    initialKind?: CredentialKind
  }>(),
  { credential: null, initialKind: 'password' }
)

const emit = defineEmits<{
  (e: 'close'): void
  /** 保存成功（回传后端摘要，调用方刷新/选中） */
  (e: 'saved', summary: CredentialSummary): void
}>()

const ui = useUiStore()

/** 表单状态（打开时按 credential/initialKind 重建） */
const state = reactive<CredentialFormState>(emptyFormState())
/** 提交中与校验错误 */
const saving = ref(false)
const error = ref('')

/** 类型下拉选项 */
const kindOptions = CREDENTIAL_KINDS.map((k) => ({ value: k, label: KIND_LABEL[k] }))

/** 打开时重建表单（编辑 = reveal 明文预填；新建 = 空白） */
watch(
  () => props.open,
  (open) => {
    if (!open) return
    const fresh = props.credential
      ? formStateFromCredential(props.credential)
      : emptyFormState(props.initialKind)
    Object.assign(state, fresh)
    error.value = ''
  }
)

/** 切换类型：重建字段值（保留名称/备注），避免残留他类型字段 */
function onKindChange(kind: string) {
  const next = emptyFormState(kind as CredentialKind)
  next.name = state.name
  next.note = state.note
  Object.assign(state, next)
}

/** 自定义条目增删 */
function addEntry() {
  state.entries.push({ key: '', value: '', secret: true })
}
function removeEntry(index: number) {
  state.entries.splice(index, 1)
  if (!state.entries.length) addEntry()
}

/** 保存：前端校验 → vault_save → toast + saved 事件 */
async function save() {
  const invalid = validateFormState(state)
  if (invalid) {
    error.value = invalid
    return
  }
  saving.value = true
  error.value = ''
  try {
    const summary = await ipc.vaultSave(payloadFromFormState(state, props.credential?.id ?? null))
    ui.toast(props.credential ? '凭证已更新' : '凭证已创建')
    emit('saved', summary)
    emit('close')
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <UiModal
    :open="open"
    :title="credential ? `编辑凭证 · ${credential.name}` : '新建凭证'"
    description="秘密加密存储在本机凭证库，插件只引用 id，明文不出后端"
    size="md"
    @close="emit('close')"
  >
    <div class="flex flex-col gap-sm">
      <div class="grid grid-cols-2 gap-sm">
        <label class="field-label flex flex-col gap-[6px]">
          类型
          <UiSelect
            :model-value="state.kind"
            :options="kindOptions"
            :disabled="Boolean(credential)"
            @update:model-value="onKindChange"
          />
        </label>
        <label class="field-label flex flex-col gap-[6px]">
          名称
          <UiInput
            :model-value="state.name"
            placeholder="如：生产 MySQL / 腾讯云 CAM"
            @update:model-value="state.name = String($event)"
          />
        </label>
      </div>

      <!-- 按 kind 动态渲染的字段（秘密用 type=password，明文切换走输入框自带眼睛；私钥多行不脱敏） -->
      <div v-for="f in formFieldsFor(state.kind)" :key="f.key" class="flex flex-col gap-[6px]">
        <span class="field-label">
          {{ f.label }}
          <span v-if="f.optional" class="text-text-muted dark:text-text-muted-dark">（可选）</span>
        </span>
        <!-- 多行秘密（私钥）：不脱敏直接可编辑（PEM 需要全文可见核对） -->
        <UiTextarea
          v-if="f.multiline"
          :model-value="state.values[f.key]"
          class="font-mono"
          resize="vertical"
          placeholder="粘贴 PEM / OpenSSH 私钥全文"
          @update:model-value="state.values[f.key] = $event"
        />
        <UiInput
          v-else
          :model-value="state.values[f.key]"
          :type="f.secret ? 'password' : 'text'"
          @update:model-value="state.values[f.key] = String($event)"
        />
      </div>

      <!-- custom：键值条目编辑器 -->
      <div v-if="state.kind === 'custom'" class="flex flex-col gap-[6px]">
        <span class="field-label">字段</span>
        <div v-for="(entry, i) in state.entries" :key="i" class="flex items-center gap-[6px]">
          <UiInput
            :model-value="entry.key"
            class="w-[140px] shrink-0"
            placeholder="键名"
            @update:model-value="entry.key = String($event)"
          />
          <UiInput
            :model-value="entry.value"
            :type="entry.secret ? 'password' : 'text'"
            class="flex-1"
            placeholder="值"
            @update:model-value="entry.value = String($event)"
          />
          <label
            class="flex shrink-0 cursor-pointer items-center gap-[4px] text-caption text-secondary dark:text-secondary-dark"
            title="秘密值在列表中掩码"
          >
            <UiCheckbox
              :model-value="entry.secret"
              @update:model-value="entry.secret = Boolean($event)"
            />
            秘密
          </label>
          <UiIconButton label="删除字段" size="xs" @click="removeEntry(i)">
            <UiIcon name="trash" :size="13" />
          </UiIconButton>
        </div>
        <UiButton variant="ghost" size="sm" class="self-start" @click="addEntry">
          + 添加字段
        </UiButton>
      </div>

      <label class="field-label flex flex-col gap-[6px]">
        备注（可选）
        <UiInput
          :model-value="state.note"
          placeholder="用途、环境等"
          @update:model-value="state.note = String($event)"
        />
      </label>

      <UiAlert v-if="error" tone="danger" size="sm">{{ error }}</UiAlert>
    </div>

    <template #footer>
      <UiButton @click="emit('close')">取消</UiButton>
      <UiButton variant="primary" :loading="saving" @click="save">
        {{ credential ? '保存' : '创建' }}
      </UiButton>
    </template>
  </UiModal>
</template>
