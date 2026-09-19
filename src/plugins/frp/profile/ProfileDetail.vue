<script setup lang="ts">
import { UiTooltip } from '@/core/ui'
/**
 * ProfileDetail · 档案详情（工具栏 + 配置 / 日志两个页签）
 * 配置页签内再分「表单 / 源码」两种模式；保存与校验由 useProfileEditor 统一处理。
 * 未保存改动通过 `update:dirty` 上报，由工作台在切换档案前拦截确认。
 */
import { computed, inject, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { UiBadge, UiButton, UiEmptyState, UiIcon, UiSpinner, UiTabs, UiToolbar } from '@/core/ui'
import type { FrpRuntimeState } from '../contracts'
import ClientPicker from '../client/ClientPicker.vue'
import { FRP_CLIENTS_KEY } from '../client/context'
import RuntimeLogPanel from '../runtime/RuntimeLogPanel.vue'
import { statusView, type FrpLogLine } from '../runtime/frpStatus'
import ProfileFormEditor from './ProfileFormEditor.vue'
import ProfileSourceEditor from './ProfileSourceEditor.vue'
import { useProfileEditor } from './useProfileEditor'

const props = defineProps<{
  /** 当前档案文件名 */
  fileName: string
  /** 该档案的运行状态（未运行过为 undefined） */
  state: FrpRuntimeState | undefined
  /** 该档案是否正在启停中 */
  busy: boolean
  /** 该档案的日志行 */
  logs: FrpLogLine[]
  /** 该档案绑定的客户端 id（未绑定时不出现 = 跟随默认） */
  clientId?: string
}>()
const emit = defineEmits<{
  start: [fileName: string]
  stop: [fileName: string]
  restart: [fileName: string]
  clearLogs: [fileName: string]
  /** 脏状态变化（工作台据此拦截切换） */
  'update:dirty': [dirty: boolean]
  /** 保存或校验成功（工作台刷新列表元数据） */
  changed: []
}>()

const { t } = useI18n()
const editor = useProfileEditor()
// 客户端清单由工作台注入；单独渲染本组件（无 provider）时退化为不显示选择器
const clientsStore = inject(FRP_CLIENTS_KEY, null)

/** 当前页签：配置 / 日志 */
const tab = ref<'config' | 'log'>('config')
/** 配置页签内的编辑模式 */
const mode = ref<'form' | 'source'>('form')

/** 状态展示视图（未运行过按 stopped 展示） */
const view = computed(() => statusView(props.state?.state ?? 'stopped'))
/** 是否运行中（日志页签与停止按钮用） */
const running = computed(
  () =>
    props.state?.pid != null ||
    props.state?.state === 'running' ||
    props.state?.state === 'starting'
)
const editingBusy = computed(
  () => editor.loading.value || editor.saving.value || editor.verifying.value
)
const needsSave = computed(() => editor.dirty.value || editingBusy.value)

/** 切换档案：重新加载内容（脏标记由 editor.load 内部重置） */
watch(
  () => props.fileName,
  (name) => {
    if (name !== '') void editor.load(name)
  },
  { immediate: true }
)

/** 脏标记上报 */
watch(
  () => editor.dirty.value,
  (value) => emit('update:dirty', value)
)

/** 保存（按当前模式分派） */
async function onSave() {
  const ok = mode.value === 'form' ? await editor.saveForm() : await editor.saveText()
  if (ok) emit('changed')
}

/** 校验 */
async function onVerify() {
  await editor.verify()
}

/**
 * 切换本档案绑定的客户端。空串表示回到「跟随默认」。
 * 写库成功后让工作台刷新档案列表，否则左栏与详情仍显示旧绑定。
 */
async function onClientChange(clientId: string) {
  if (clientsStore === null) return
  const ok = await clientsStore.bindProfile(props.fileName, clientId === '' ? undefined : clientId)
  if (ok) emit('changed')
}

/** 表单模式保存前的注释风险提示（有注释时二次确认由工作台 toast 兜底，这里只用文案提示） */
const commentWarning = computed(() => mode.value === 'form' && editor.hasComments.value)
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <!-- 工具栏 -->
    <UiToolbar bordered>
      <UiTooltip :content="props.fileName">
        <span class="min-w-0 max-w-[240px] truncate text-body font-medium dark:text-primary-dark">
          {{ props.fileName }}
        </span>
      </UiTooltip>
      <UiBadge :tone="view.tone">{{ t(view.labelKey) }}</UiBadge>
      <span
        v-if="editor.dirty.value"
        class="shrink-0 text-caption text-tertiary-strong dark:text-tertiary-dark"
      >
        {{ t('frp.unsaved') }}
      </span>

      <template #trailing>
        <UiButton
          v-if="!running"
          :aria-label="t('frp.actionStart')"
          size="sm"
          variant="primary"
          :disabled="props.busy || needsSave"
          @click="emit('start', props.fileName)"
        >
          <UiIcon name="play" :size="14" />
          {{ t('frp.actionStart') }}
        </UiButton>
        <UiButton
          v-else
          :aria-label="t('frp.actionStop')"
          size="sm"
          class="text-danger-strong hover:bg-danger-soft dark:text-danger-dark"
          :disabled="props.busy"
          @click="emit('stop', props.fileName)"
        >
          <UiIcon name="stop" :size="14" />
          {{ t('frp.actionStop') }}
        </UiButton>
        <UiButton
          :aria-label="t('frp.actionRestart')"
          size="sm"
          variant="secondary"
          :disabled="props.busy || !running || needsSave"
          @click="emit('restart', props.fileName)"
        >
          {{ t('frp.actionRestart') }}
        </UiButton>
        <UiButton size="sm" variant="secondary" :disabled="needsSave" @click="onVerify">
          {{ editor.verifying.value ? t('frp.verifying') : t('frp.actionVerify') }}
        </UiButton>
        <UiButton size="sm" :disabled="editingBusy || !editor.dirty.value" @click="onSave">
          {{ editor.saving.value ? t('frp.saving') : t('frp.actionSave') }}
        </UiButton>
      </template>
    </UiToolbar>

    <!-- 错误行（加载失败等，始终可见） -->
    <p
      v-if="editor.error.value !== ''"
      class="shrink-0 border-b border-border bg-danger-soft px-[10px] py-[5px] text-body-sm text-danger-strong dark:border-border-dark dark:bg-danger-soft-dark dark:text-danger-dark"
    >
      {{ editor.error.value }}
    </p>

    <!-- 编辑导航与客户端集中在同一行，窄窗口自动换行 -->
    <UiToolbar bordered>
      <UiTabs
        size="sm"
        :model-value="tab"
        :items="[
          { value: 'config', label: t('frp.tabConfig') },
          { value: 'log', label: t('frp.tabLog') },
        ]"
        @update:model-value="tab = $event as 'config' | 'log'"
      />

      <div v-if="tab === 'config'" class="flex items-center gap-[4px]">
        <UiButton
          size="xs"
          :variant="mode === 'form' ? 'primary' : 'secondary'"
          :disabled="needsSave && mode !== 'form'"
          @click="mode = 'form'"
        >
          {{ t('frp.modeForm') }}
        </UiButton>
        <UiButton
          size="xs"
          :variant="mode === 'source' ? 'primary' : 'secondary'"
          :disabled="needsSave && mode !== 'source'"
          @click="mode = 'source'"
        >
          {{ t('frp.modeSource') }}
        </UiButton>
      </div>
      <template v-if="clientsStore !== null" #trailing>
        <ClientPicker
          :clients="clientsStore.clients.value"
          :default-id="clientsStore.defaultId.value"
          :bound-id="props.clientId"
          :disabled="props.busy || running"
          @change="onClientChange"
        />
      </template>
    </UiToolbar>
    <p
      v-if="commentWarning && tab === 'config'"
      class="px-[16px] py-[4px] text-caption text-warning-strong dark:text-warning-dark"
    >
      {{ t('frp.formCommentWarning') }}
    </p>

    <!-- 配置页签 -->
    <div v-if="tab === 'config'" class="flex min-h-0 flex-1 flex-col">
      <p
        v-if="editor.dirty.value"
        class="px-[10px] py-[4px] text-caption text-text-muted dark:text-text-muted-dark"
      >
        {{ t('frp.saveBeforeAction') }}
      </p>
      <div v-if="editor.loading.value" class="flex flex-1 items-center justify-center">
        <UiSpinner />
      </div>
      <ProfileFormEditor
        v-else-if="mode === 'form'"
        class="min-h-0 flex-1"
        :model-value="editor.model.value"
        @update:model-value="editor.updateModel"
      />
      <ProfileSourceEditor
        v-else
        class="min-h-0 flex-1"
        :model-value="editor.content.value"
        :file-name="props.fileName"
        :errors="editor.verifyErrors.value"
        :verified="editor.verified.value"
        @update:model-value="editor.updateContent"
      />
    </div>

    <!-- 日志页签 -->
    <RuntimeLogPanel
      v-else
      class="min-h-0 flex-1"
      :lines="props.logs"
      :running="running"
      @clear="emit('clearLogs', props.fileName)"
    />

    <!-- 无档案时的占位（工作台在无选中时也会渲染本组件，这里兜底） -->
    <UiEmptyState v-if="props.fileName === ''" class="flex-1" :title="t('frp.noProfileSelected')" />
  </div>
</template>
