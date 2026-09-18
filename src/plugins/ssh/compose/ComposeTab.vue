<script setup lang="ts">
/** 文件中心的编排工作区：运行操作、容器与就地 YAML 编辑。 */
import { computed, onUnmounted, ref, watch } from 'vue'
import {
  UiButton,
  UiCheckbox,
  UiCodeEditor,
  UiCombobox,
  UiEmptyState,
  UiModal,
  UiScrollArea,
  UiSelect,
} from '@/core/ui'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import type { ComposeAction, ComposeProject, ServerConnection } from '../contracts'
import { useCompose } from './useCompose'
import { composeStatus, composeTemplates, parentDirectory } from './composeTemplates'
import ComposeCreateDialog from './ComposeCreateDialog.vue'
import ComposeContainers from './ComposeContainers.vue'
import ComposeProjectList from './ComposeProjectList.vue'

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
  liveOutput,
  connected,
  dirty,
  busy,
  refresh,
  open,
  create,
  save,
  run,
  discard,
  defaultDirectory,
  rememberDirectory,
} = useCompose(
  () => props.connection,
  () => props.profileId,
  props.workspaceId
)
const showList = ref(true)
const editing = ref(false)
const expanded = ref(false)
const showCreate = ref(false)
const outputOpen = ref(false)
const confirmDown = ref(false)
const confirmRebuild = ref(false)
const buildImage = ref(false)
const pendingDiscard = ref<(() => void) | null>(null)
const containers = ref<InstanceType<typeof ComposeContainers>>()
let previousProject: ComposeProject | null = null
const projectOptions = computed(() => projects.value.map((p) => ({ value: p.name, label: p.name })))
const fileOptions = computed(() =>
  (selected.value?.configFiles ?? []).map((path) => ({
    value: path,
    label: path.split('/').pop() || path,
  }))
)
const status = computed(() => composeStatus(selected.value?.status ?? ''))
const pendingProject = ref<ComposeProject | null>(null)
const operationProjectName = ref('')
const resultText = computed(
  () =>
    liveOutput.value ||
    (output.value
      ? [output.value.stdout, output.value.stderr].filter(Boolean).join('\n') ||
        '命令执行完成，无输出。'
      : '正在等待远程输出…')
)

function navigate(callback: () => void) {
  if (busy.value) return
  if (dirty.value) pendingDiscard.value = callback
  else callback()
}
function acceptDiscard() {
  const callback = pendingDiscard.value
  pendingDiscard.value = null
  callback?.()
}
function selectProject(project: ComposeProject, path?: string, edit = false) {
  navigate(() => {
    showList.value = false
    editing.value = edit
    expanded.value = false
    void open(project, path)
  })
}
function add() {
  navigate(() => {
    previousProject = showList.value ? null : selected.value
    create()
    content.value = composeTemplates.find((t) => t.value === 'nginx')!.content
    showCreate.value = true
  })
}
function closeCreate() {
  if (busy.value) return
  discard()
  showCreate.value = false
  if (previousProject) void open(previousProject)
}
async function createFile(name: string, path: string, base: string) {
  draftName.value = name
  filePath.value = path
  const saved = await save()
  if (!isNew.value) {
    showList.value = false
    showCreate.value = false
    editing.value = false
  }
  if (!saved) return
  await rememberDirectory(base)
}
async function saveFile() {
  if (await save()) {
    editing.value = false
  }
}
async function execute(next: ComposeAction, project = selected.value) {
  if (!project) return
  operationProjectName.value = project.name
  outputOpen.value = true
  await run(next, project)
  if (next !== 'config') void containers.value?.refresh()
}
function cancelEdit() {
  navigate(() => {
    discard()
    editing.value = false
  })
}
watch([dirty, busy], () => emit('state', { dirty: dirty.value, busy: busy.value }), {
  immediate: true,
  flush: 'sync',
})
onUnmounted(() => emit('state', { dirty: false, busy: false }))
function requestAction(project: ComposeProject, next: ComposeAction) {
  if (busy.value || !connected.value) return
  pendingProject.value = project
  if (next === 'recreate') {
    buildImage.value = false
    confirmRebuild.value = true
  } else if (next === 'down') confirmDown.value = true
  else void execute(next, project)
}
function rebuild() {
  confirmRebuild.value = false
  void execute(buildImage.value ? 'rebuild' : 'recreate', pendingProject.value)
}
function down() {
  confirmDown.value = false
  void execute('down', pendingProject.value)
}
function backToList() {
  navigate(() => {
    discard()
    editing.value = expanded.value = false
    showList.value = true
  })
}
function switchProject(name: string) {
  if (name === selected.value?.name) return
  const project = projects.value.find((p) => p.name === name)
  if (project) selectProject(project)
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <ComposeProjectList
      v-show="showList"
      :projects="projects"
      :loading="listing"
      :disabled="busy"
      :connected="connected"
      :error="listError"
      :busy-project-name="action ? operationProjectName : undefined"
      @action="requestAction"
      @edit="selectProject($event, undefined, true)"
      @select="selectProject"
      @add="add"
      @refresh="refresh"
    />
    <UiScrollArea v-show="!showList" class="flex min-h-0 min-w-0 flex-1 flex-col" axis="vertical">
      <p
        v-if="!connected"
        role="alert"
        class="px-md py-sm text-body-sm text-warning-strong dark:text-warning-dark"
      >
        SSH 已断开，编辑草稿保留；恢复连接后请重新读取文件。
      </p>
      <p
        v-if="listError"
        role="alert"
        class="px-md py-sm text-body-sm text-danger-strong dark:text-danger-dark"
      >
        {{ listError }}
      </p>
      <div
        v-if="selected && !showList"
        class="flex shrink-0 flex-wrap items-center gap-sm border-b border-border px-md py-sm dark:border-border-dark"
      >
        <UiButton size="sm" variant="ghost" :disabled="busy" @click="backToList">返回列表</UiButton>
        <UiCombobox
          :model-value="selected.name"
          :options="projectOptions"
          class="w-[240px] max-w-full"
          size="sm"
          :disabled="busy"
          placeholder="切换编排"
          search-placeholder="搜索编排名称"
          @update:model-value="switchProject"
        />
        <span class="text-body-sm text-secondary dark:text-secondary-dark">{{ status }}</span>
      </div>
      <template v-if="selected && !showCreate && !showList">
        <div
          v-show="!expanded"
          class="shrink-0 border-b border-border px-md py-sm dark:border-border-dark"
        >
          <p class="select-text break-all font-mono text-body-sm">{{ filePath }}</p>
          <p class="mt-xs text-caption text-text-muted dark:text-text-muted-dark">
            工作目录：<span class="select-text">{{
              selected.workingDir || parentDirectory(selected.configFiles[0] || '/')
            }}</span
            ><span v-if="!selected.workingDir"> · 未记录原目录，按首个配置文件目录处理</span>
          </p>
        </div>
        <ComposeContainers
          v-show="!expanded"
          ref="containers"
          :project="selected"
          :connection="connection"
          :busy="busy"
        />
        <section class="flex min-h-[260px] shrink-0 flex-1 flex-col">
          <div class="flex shrink-0 flex-wrap items-center gap-xs px-md py-sm">
            <UiSelect
              v-if="fileOptions.length > 1"
              :model-value="filePath"
              :options="fileOptions"
              size="sm"
              class="!w-[180px]"
              :disabled="busy"
              @update:model-value="selectProject(selected!, String($event))"
            />
            <span v-else class="text-body-sm font-medium">{{ filePath.split('/').pop() }}</span>
            <span v-if="dirty" class="text-caption text-warning-strong dark:text-warning-dark"
              >未保存</span
            >
            <div class="ml-auto flex flex-wrap gap-xs">
              <template v-if="editing">
                <UiButton
                  size="xs"
                  variant="ghost"
                  :disabled="busy || !connected"
                  @click="execute('config')"
                  >校验</UiButton
                >
                <UiButton size="xs" variant="ghost" :disabled="busy" @click="cancelEdit"
                  >取消</UiButton
                >
                <UiButton
                  size="xs"
                  variant="secondary"
                  :loading="saving"
                  :disabled="busy || !connected || !dirty"
                  @click="saveFile()"
                  >保存</UiButton
                >
              </template>
              <UiButton
                v-else
                size="xs"
                variant="secondary"
                :disabled="busy || !loaded || !connected"
                @click="editing = true"
                >编辑</UiButton
              >
              <UiButton
                size="xs"
                variant="ghost"
                :disabled="busy || !connected"
                @click="selectProject(selected!, filePath)"
                >重新读取</UiButton
              >
              <UiButton size="xs" variant="ghost" @click="expanded = !expanded">{{
                expanded ? '收起' : '展开'
              }}</UiButton>
            </div>
          </div>
          <!-- 为百分比高度编辑器提供确定的承载区域，避免被容器列表与输出挤到零高。 -->
          <div v-if="loaded" class="relative min-h-[200px] flex-1">
            <UiCodeEditor
              v-model="content"
              class="absolute inset-0"
              :filename="filePath"
              language="yaml"
              placeholder="当前 YAML 文件内容为空"
              :readonly="!editing || busy"
              @error="error = $event"
              @save="editing && !busy && saveFile()"
            />
          </div>
          <p v-else class="px-md py-sm text-body-sm text-text-muted dark:text-text-muted-dark">
            {{ loading ? '正在读取配置…' : '配置未加载，请检查路径和权限后重新读取。' }}
          </p>
        </section>
      </template>
      <UiEmptyState
        v-else
        class="flex-1"
        title="选择容器编排"
        description="选择已有编排，或添加 Compose 文件。"
      />
    </UiScrollArea>
    <p
      v-if="error && !showCreate"
      role="alert"
      class="shrink-0 select-text px-md py-sm text-body-sm text-danger-strong dark:text-danger-dark"
    >
      {{ error }}
    </p>
    <p
      v-if="notice && !showCreate"
      role="status"
      class="shrink-0 px-md py-xs text-body-sm text-secondary dark:text-secondary-dark"
    >
      {{ notice }}
    </p>
    <div
      v-if="action || output || liveOutput"
      class="shrink-0 border-t border-border dark:border-border-dark"
    >
      <div class="flex items-center justify-between px-md py-xs">
        <span class="text-body-sm"
          >{{ operationProjectName }} · {{ action ? '正在执行…' : '执行结果' }}</span
        ><UiButton size="xs" variant="ghost" @click="outputOpen = !outputOpen">{{
          outputOpen ? '收起输出' : '查看输出'
        }}</UiButton>
      </div>
      <UiScrollArea v-if="outputOpen" class="max-h-[150px]" axis="both">
        <pre class="select-text whitespace-pre-wrap break-all px-md pb-sm font-mono text-body-sm">{{
          resultText
        }}</pre>
      </UiScrollArea>
    </div>
    <ComposeCreateDialog
      v-if="showCreate && connection"
      v-model:content="content"
      :connection-id="connection.sessionId"
      :busy="busy"
      :connected="connected"
      :error="error"
      :default-directory="defaultDirectory"
      @close="closeCreate"
      @save="createFile"
    />
    <ConfirmDialog
      :open="pendingDiscard !== null"
      title="放弃未保存的修改"
      message="当前 YAML 尚未保存，继续将放弃这些修改。"
      confirm-label="放弃修改"
      @close="pendingDiscard = null"
      @confirm="acceptDiscard"
    />
    <ConfirmDialog
      :open="confirmDown"
      title="拆除编排"
      :message="`将停止并移除 ${pendingProject?.name} 的容器与默认网络，保留 YAML 和数据卷。`"
      confirm-label="拆除"
      danger
      @close="confirmDown = false"
      @confirm="down"
    />
    <UiModal
      :open="confirmRebuild"
      :title="`重建容器 · ${pendingProject?.name ?? ''}`"
      @close="confirmRebuild = false"
    >
      <p class="mb-md text-body-sm">使用已保存配置重新创建容器，保留数据卷。</p>
      <UiCheckbox v-model="buildImage" label="同时重新构建镜像（配置中包含 build 时）" />
      <template #footer
        ><UiButton variant="ghost" @click="confirmRebuild = false">取消</UiButton
        ><UiButton @click="rebuild">重建</UiButton></template
      >
    </UiModal>
  </div>
</template>
