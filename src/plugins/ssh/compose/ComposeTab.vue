<script setup lang="ts">
/** 编排列表负责运行操作；容器查看与文件编辑分别在独立弹窗中按需加载。 */
import { computed, onUnmounted, ref, watch } from 'vue'
import { UiButton, UiCheckbox, UiCodeEditor, UiModal, UiScrollArea, UiSelect } from '@/core/ui'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import type { ComposeAction, ComposeProject, ServerConnection } from '../contracts'
import { useCompose } from './useCompose'
import { composeTemplates } from './composeTemplates'
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
const showEditor = ref(false)
const viewingProject = ref<ComposeProject | null>(null)
const showCreate = ref(false)
const outputOpen = ref(false)
const confirmDown = ref(false)
const confirmRebuild = ref(false)
const buildImage = ref(false)
const pendingDiscard = ref<(() => void) | null>(null)
const fileOptions = computed(() =>
  (selected.value?.configFiles ?? []).map((path) => ({
    value: path,
    label: path,
  }))
)
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
function editProject(project: ComposeProject, path?: string) {
  navigate(() => {
    showEditor.value = true
    void open(project, path)
  })
}
function add() {
  navigate(() => {
    create()
    content.value = composeTemplates.find((t) => t.value === 'nginx')!.content
    showCreate.value = true
  })
}
function closeCreate() {
  if (busy.value) return
  discard()
  showCreate.value = false
}
async function createFile(name: string, path: string, base: string) {
  draftName.value = name
  filePath.value = path
  const saved = await save()
  if (!isNew.value) {
    showCreate.value = false
  }
  if (!saved) return
  await rememberDirectory(base)
}
async function saveFile() {
  if (!dirty.value) return
  await save()
}
async function execute(next: ComposeAction, project = selected.value) {
  if (!project) return
  operationProjectName.value = project.name
  outputOpen.value = true
  await run(next, project)
}
function closeEditor() {
  navigate(() => {
    discard()
    showEditor.value = false
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
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <ComposeProjectList
      :projects="projects"
      :loading="listing"
      :disabled="busy"
      :connected="connected"
      :error="listError"
      :busy-project-name="action ? operationProjectName : undefined"
      @action="requestAction"
      @edit="editProject"
      @view="viewingProject = $event"
      @add="add"
      @refresh="refresh"
    />
    <UiModal
      v-if="viewingProject"
      open
      size="xl"
      :title="`${viewingProject.name} · 容器`"
      @close="viewingProject = null"
    >
      <ComposeContainers :project="viewingProject" :connection="connection" :busy="busy" />
    </UiModal>
    <UiModal
      v-if="showEditor"
      open
      size="xl"
      :title="`${selected?.name ?? ''} · 编辑`"
      @close="closeEditor"
    >
      <UiSelect
        v-if="fileOptions.length > 1"
        :model-value="filePath"
        :options="fileOptions"
        :disabled="busy || loading"
        class="mb-sm"
        @update:model-value="editProject(selected!, String($event))"
      />
      <p
        v-else
        class="mb-sm select-text break-all font-mono text-body-sm text-secondary dark:text-secondary-dark"
      >
        {{ filePath }}
      </p>
      <div class="relative h-[55vh] min-h-[240px]">
        <UiCodeEditor
          v-if="loaded"
          v-model="content"
          class="absolute inset-0"
          language="yaml"
          :filename="filePath"
          :readonly="busy"
          @save="saveFile"
          @error="error = $event"
        />
        <p v-else class="text-body-sm text-text-muted dark:text-text-muted-dark">
          {{ loading ? '正在读取文件…' : '配置未加载' }}
        </p>
      </div>
      <p
        v-if="error"
        role="alert"
        class="mt-sm select-text text-body-sm text-danger-strong dark:text-danger-dark"
      >
        {{ error }}
      </p>
      <p
        v-if="notice"
        role="status"
        class="mt-sm text-body-sm text-secondary dark:text-secondary-dark"
      >
        {{ notice }}
      </p>
      <template #footer>
        <UiButton variant="ghost" :disabled="busy" @click="closeEditor">关闭</UiButton>
        <UiButton
          :loading="saving"
          :disabled="busy || !connected || !loaded || !dirty"
          @click="saveFile"
          >保存</UiButton
        >
      </template>
      <ConfirmDialog
        :open="pendingDiscard !== null"
        title="放弃未保存的修改"
        message="当前 YAML 有未保存修改，是否放弃？"
        confirm-label="放弃"
        @close="pendingDiscard = null"
        @confirm="acceptDiscard"
      />
    </UiModal>
    <p
      v-if="error && !showCreate && !showEditor"
      role="alert"
      class="shrink-0 select-text px-md py-sm text-body-sm text-danger-strong dark:text-danger-dark"
    >
      {{ error }}
    </p>
    <p
      v-if="notice && !showCreate && !showEditor"
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
