<script setup lang="ts">
/** Compose 项目列表与 YAML 工作区；隐藏页签保留草稿，远程操作只由显式按钮触发。 */
import { computed, onUnmounted, ref, watch } from 'vue'
import {
  UiButton,
  UiCodeEditor,
  UiEmptyState,
  UiField,
  UiInput,
  UiScrollArea,
  UiSearchInput,
  UiSelect,
  UiSpinner,
} from '@/core/ui'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import type { ComposeAction, ServerConnection } from '../contracts'
import { useCompose } from './useCompose'

const props = defineProps<{
  connection?: ServerConnection
  profileId: string
  workspaceId: string
}>()
const emit = defineEmits<{ state: [value: { dirty: boolean; busy: boolean }] }>()
const {
  projects,
  selected,
  filePath,
  draftName,
  content,
  isNew,
  loaded,
  loading,
  listing,
  saving,
  action,
  error,
  listError,
  notice,
  output,
  connected,
  dirty,
  busy,
  mtime,
  refresh,
  open,
  create,
  save,
  run,
} = useCompose(
  () => props.connection,
  () => props.profileId,
  props.workspaceId
)
const keyword = ref('')
const chosenAction = ref<ComposeAction>('ps')
const pendingDiscard = ref<(() => void) | null>(null)
const confirmDown = ref(false)
const filtered = computed(() =>
  projects.value.filter((project) =>
    `${project.name} ${project.configFiles.join(' ')}`
      .toLowerCase()
      .includes(keyword.value.trim().toLowerCase())
  )
)
const fileOptions = computed(() =>
  (selected.value?.configFiles ?? []).map((path) => ({ value: path, label: path }))
)
const resultText = computed(() =>
  output.value
    ? [output.value.stdout, output.value.stderr].filter(Boolean).join('\n') ||
      '命令执行完成，无输出。'
    : ''
)
const actionOptions = [
  { value: 'ps', label: '查看服务状态' },
  { value: 'logs', label: '查看日志 · 最近 200 行' },
  { value: 'config', label: '校验配置' },
  { value: 'start', label: '启动已有容器' },
  { value: 'stop', label: '停止项目' },
  { value: 'restart', label: '重启项目' },
  { value: 'pull', label: '拉取镜像' },
  { value: 'build', label: '构建镜像' },
  { value: 'down', label: '拆除项目' },
]
const cannotRun = computed(
  () =>
    !connected.value ||
    !selected.value?.configFiles.length ||
    busy.value ||
    loading.value ||
    dirty.value
)

function navigate(callback: () => void) {
  if (busy.value) return
  if (dirty.value) pendingDiscard.value = callback
  else callback()
}
function discard() {
  const callback = pendingDiscard.value
  pendingDiscard.value = null
  callback?.()
}
function execute() {
  if (chosenAction.value === 'down') confirmDown.value = true
  else void run(chosenAction.value)
}
function down() {
  confirmDown.value = false
  void run('down')
}
watch([dirty, busy], () => emit('state', { dirty: dirty.value, busy: busy.value }), {
  immediate: true,
  flush: 'sync',
})
onUnmounted(() => emit('state', { dirty: false, busy: false }))
</script>

<template>
  <div class="flex h-full min-h-0">
    <aside class="flex w-[220px] shrink-0 flex-col border-r border-border dark:border-border-dark">
      <div class="flex items-center justify-between gap-[6px] px-[10px] py-[8px]">
        <span class="text-body-sm font-medium text-primary dark:text-primary-dark"
          >Compose 项目</span
        >
        <UiButton
          variant="ghost"
          size="xs"
          :loading="listing"
          :disabled="!connected"
          @click="refresh"
          >刷新</UiButton
        >
      </div>
      <UiSearchInput
        v-model="keyword"
        class="mx-[10px] mb-[8px] !w-auto"
        size="sm"
        placeholder="搜索项目或路径"
      />
      <UiScrollArea class="min-h-0 flex-1" axis="vertical">
        <div class="space-y-[4px] p-[6px]">
          <UiButton
            v-for="project in filtered"
            :key="project.name"
            variant="ghost"
            size="sm"
            block
            class="!h-auto !justify-start !whitespace-normal !py-[9px] text-left"
            :class="
              selected?.name === project.name && !isNew
                ? '!bg-tertiary-soft dark:!bg-tertiary-soft-dark'
                : ''
            "
            :disabled="busy"
            @click="
              navigate(() => {
                void open(project)
              })
            "
          >
            <span class="min-w-0"
              ><span class="block break-all font-medium">{{ project.name }}</span
              ><span
                class="mt-[3px] block break-words text-caption text-text-muted dark:text-text-muted-dark"
                >{{ project.status }}</span
              ></span
            >
          </UiButton>
          <p
            v-if="!filtered.length"
            class="p-[8px] text-body-sm text-text-muted dark:text-text-muted-dark"
          >
            {{ listing ? '正在查询项目…' : '暂无匹配项目，可新建配置。' }}
          </p>
        </div>
      </UiScrollArea>
      <div class="border-t border-border p-[10px] dark:border-border-dark">
        <UiButton
          size="sm"
          variant="secondary"
          block
          :disabled="!connected || busy"
          @click="navigate(create)"
          >新建 Compose 配置</UiButton
        >
      </div>
    </aside>

    <div class="flex min-w-0 flex-1 flex-col">
      <p
        v-if="!connected"
        role="alert"
        class="px-[14px] py-[8px] text-body-sm text-warning-strong dark:text-warning-dark"
      >
        SSH 已断开，草稿保留在本页；恢复连接后请重新读取文件版本。
      </p>
      <p
        v-if="listError"
        role="alert"
        class="select-text px-[14px] py-[8px] text-body-sm text-danger-strong dark:text-danger-dark"
      >
        项目查询失败：{{ listError }}
      </p>
      <UiEmptyState
        v-if="!selected && !isNew"
        class="m-auto"
        title="选择一个 Compose 项目"
        description="从 Docker 查询项目与配置路径，或新建 YAML 配置后部署。"
      />
      <template v-else>
        <header
          class="flex shrink-0 flex-wrap items-center gap-[8px] border-b border-border px-[12px] py-[8px] dark:border-border-dark"
        >
          <h2 class="mr-auto text-body font-medium text-primary dark:text-primary-dark">
            {{ isNew ? '新建配置' : selected?.name }}
          </h2>
          <template v-if="!isNew">
            <UiSelect
              :model-value="chosenAction"
              :options="actionOptions"
              size="sm"
              class="!w-[175px]"
              :disabled="busy"
              title="Compose 操作"
              @update:model-value="chosenAction = $event as ComposeAction"
            />
            <UiButton
              size="sm"
              variant="secondary"
              :disabled="cannotRun"
              :loading="action !== null && action !== 'up'"
              @click="execute"
              >执行</UiButton
            >
            <UiButton size="sm" :disabled="cannotRun" :loading="action === 'up'" @click="run('up')"
              >启动 / 更新</UiButton
            >
          </template>
        </header>
        <div v-if="isNew" class="grid shrink-0 grid-cols-2 gap-[12px] p-[12px]">
          <UiField label="项目名称"
            ><UiInput v-model="draftName" size="sm" placeholder="my-app" :disabled="busy"
          /></UiField>
          <UiField label="远程配置路径"
            ><UiInput
              v-model="filePath"
              size="sm"
              placeholder="/opt/app/compose.yaml"
              :disabled="busy"
          /></UiField>
          <p class="col-span-2 text-caption text-text-muted dark:text-text-muted-dark">
            父目录须已存在；保存只创建配置，不会自动启动容器。
          </p>
        </div>
        <div
          class="flex shrink-0 flex-wrap items-center gap-[8px] border-b border-border px-[12px] py-[7px] dark:border-border-dark"
        >
          <UiSelect
            v-if="fileOptions.length"
            :model-value="filePath"
            :options="fileOptions"
            size="sm"
            class="min-w-0 flex-1"
            title="按 Compose 合并顺序排列的配置文件"
            :disabled="busy"
            @update:model-value="
              navigate(() => {
                if (selected) void open(selected, String($event))
              })
            "
          />
          <span class="text-caption text-text-muted dark:text-text-muted-dark">{{
            dirty ? '未保存' : loaded ? '与读取版本一致' : 'YAML 配置'
          }}</span>
          <UiButton
            v-if="selected"
            variant="ghost"
            size="sm"
            :disabled="!connected || busy || loading"
            @click="
              navigate(() => {
                if (selected) void open(selected, filePath)
              })
            "
            >重新读取</UiButton
          >
          <UiButton
            variant="secondary"
            size="sm"
            :loading="saving"
            :disabled="!connected || !loaded || busy || (!isNew && (!dirty || mtime === undefined))"
            @click="save"
            >{{ isNew ? '创建配置' : '保存 YAML' }}</UiButton
          >
        </div>
        <p
          v-if="error"
          role="alert"
          class="select-text px-[12px] py-[8px] text-body-sm text-danger-strong dark:text-danger-dark"
        >
          {{ error }}
        </p>
        <p
          v-if="notice"
          role="status"
          class="px-[12px] py-[6px] text-caption text-secondary dark:text-secondary-dark"
        >
          {{ notice }}
        </p>
        <div
          v-if="loading"
          class="flex min-h-[120px] flex-1 items-center justify-center gap-[8px] text-body-sm text-secondary dark:text-secondary-dark"
        >
          <UiSpinner />读取配置…
        </div>
        <UiCodeEditor
          v-else-if="loaded"
          v-model="content"
          class="min-h-[150px] flex-1"
          language="yaml"
          :filename="filePath || 'compose.yaml'"
          :readonly="busy || !connected"
          status-bar
          @save="save"
        />
        <UiEmptyState
          v-else
          title="配置尚未读取"
          description="检查远程文件是否存在及当前用户的读取权限。"
        />
        <div
          v-if="action || output"
          class="flex h-[190px] shrink-0 flex-col border-t border-border dark:border-border-dark"
        >
          <div
            class="flex items-center gap-[8px] px-[12px] py-[6px] text-caption text-secondary dark:text-secondary-dark"
          >
            <UiSpinner v-if="action" />{{
              action ? '正在执行，完成后显示输出…' : `执行结果 · 退出码 ${output?.exitCode}`
            }}
          </div>
          <UiCodeEditor
            v-if="output"
            :model-value="resultText"
            readonly
            language="text"
            class="min-h-0 flex-1"
            :line-numbers="false"
          />
        </div>
      </template>
    </div>
    <ConfirmDialog
      :open="pendingDiscard !== null"
      title="放弃未保存的修改"
      message="当前 YAML 尚未保存，继续将丢弃本页修改。"
      confirm-label="放弃修改并继续"
      @close="pendingDiscard = null"
      @confirm="discard"
    />
    <ConfirmDialog
      :open="confirmDown"
      title="拆除 Compose 项目"
      :message="`将停止并移除项目「${selected?.name ?? ''}」的容器与项目网络，保留数据卷和 YAML 配置。`"
      confirm-label="拆除项目"
      danger
      @close="confirmDown = false"
      @confirm="down"
    />
  </div>
</template>
