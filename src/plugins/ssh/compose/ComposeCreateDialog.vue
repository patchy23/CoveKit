<script setup lang="ts">
/** 新建编排的名称、远程落位和模板；写盘由工作区统一管理。 */
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { UiButton, UiCodeEditor, UiField, UiInput, UiModal, UiSelect } from '@/core/ui'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import ComposeDirectoryPicker from './ComposeDirectoryPicker.vue'
import { composePath, composeTemplates, parentDirectory } from './composeTemplates'
import { validateComposeDraft } from './composeProjects'

const props = defineProps<{
  connectionId: string
  busy: boolean
  connected: boolean
  error: string
  content: string
  defaultDirectory: () => Promise<string>
}>()
const emit = defineEmits<{
  close: []
  'update:content': [value: string]
  save: [name: string, path: string, apply: boolean, base: string]
  openExisting: [name: string, path: string]
}>()
const name = ref('')
const directory = ref('')
const filename = ref('docker-compose.yml')
const base = ref('')
const customDirectory = ref(false)
const template = ref('nginx')
const pendingTemplate = ref('')
const choosingDirectory = ref(false)
const localError = ref('')
const initializing = ref(true)
let alive = true
onUnmounted(() => {
  alive = false
})
const finalPath = computed(() => composePath(directory.value, filename.value))
watch([name, base], () => {
  if (!customDirectory.value) directory.value = composePath(base.value, name.value)
})
onMounted(async () => {
  try {
    const result = await props.defaultDirectory()
    if (alive) base.value = result
  } catch (e) {
    if (alive) localError.value = String(e)
  } finally {
    if (alive) initializing.value = false
  }
})
function applyTemplate(value: string) {
  template.value = value
  emit('update:content', composeTemplates.find((t) => t.value === value)!.content)
  pendingTemplate.value = ''
}
function selectTemplate(value: string) {
  if (value === template.value) return
  const original = composeTemplates.find((t) => t.value === template.value)!.content
  if (props.content !== original) pendingTemplate.value = value
  else applyTemplate(value)
}
function validate() {
  localError.value = !directory.value.trim().startsWith('/')
    ? '请填写远程项目目录的绝对路径'
    : filename.value.includes('/') || ['.', '..'].includes(filename.value.trim())
      ? '文件名不能包含目录，请在项目目录中填写路径'
      : validateComposeDraft(name.value.trim(), finalPath.value)
  return !localError.value
}
function save(apply: boolean) {
  if (validate())
    emit(
      'save',
      name.value.trim(),
      finalPath.value,
      apply,
      parentDirectory(directory.value.replace(/\/+$/, ''))
    )
}
function changeDirectory(value: string | number) {
  directory.value = String(value)
  customDirectory.value = true
}
function chooseDirectory(value: string) {
  changeDirectory(value)
  choosingDirectory.value = false
}
</script>

<template>
  <UiModal open title="添加容器编排" size="xl" @close="!busy && emit('close')">
    <div class="grid grid-cols-2 gap-sm">
      <UiField label="编排名称" required
        ><UiInput v-model="name" :disabled="busy" placeholder="例如 blog"
      /></UiField>
      <UiField label="模板"
        ><UiSelect
          :model-value="template"
          :options="composeTemplates"
          :disabled="busy"
          @update:model-value="selectTemplate(String($event))"
      /></UiField>
      <UiField label="远程项目目录" required>
        <div class="flex gap-sm">
          <UiInput
            :model-value="directory"
            :disabled="busy"
            :placeholder="initializing ? '正在读取远程主目录…' : '/home/user/compose/blog'"
            @update:model-value="changeDirectory"
          />
          <UiButton
            variant="secondary"
            :disabled="busy || !connected"
            @click="choosingDirectory = true"
            >选择</UiButton
          >
        </div>
      </UiField>
      <UiField label="文件名" required><UiInput v-model="filename" :disabled="busy" /></UiField>
    </div>
    <p class="my-sm break-all text-body-sm text-secondary dark:text-secondary-dark">
      最终路径：<span class="select-text font-mono">{{ finalPath }}</span>
    </p>
    <UiCodeEditor
      :model-value="content"
      language="yaml"
      :filename="filename"
      height="min(42vh, 420px)"
      :readonly="busy"
      @update:model-value="emit('update:content', $event)"
    />
    <p class="mt-sm text-caption text-text-muted dark:text-text-muted-dark">
      相对路径 ./data
      位于项目目录下。保存会创建项目目录；不会代建挂载配置文件。数据库模板需先在项目目录的 .env
      中填写提示的变量。
    </p>
    <p
      v-if="localError || error"
      role="alert"
      class="mt-sm select-text text-body-sm text-danger-strong dark:text-danger-dark"
    >
      {{ localError || error }}
    </p>
    <template #footer>
      <UiButton
        variant="ghost"
        :disabled="busy || !connected"
        @click="validate() && emit('openExisting', name.trim(), finalPath)"
        >打开已有文件</UiButton
      >
      <UiButton variant="ghost" :disabled="busy" @click="emit('close')">取消</UiButton>
      <UiButton
        variant="secondary"
        :loading="busy"
        :disabled="!connected || initializing"
        @click="save(false)"
        >保存</UiButton
      >
      <UiButton :disabled="busy || !connected || initializing" @click="save(true)"
        >保存并启动</UiButton
      >
    </template>
  </UiModal>
  <ComposeDirectoryPicker
    v-if="choosingDirectory"
    :connection-id="connectionId"
    :initial-path="base ? parentDirectory(base) : '/'"
    @close="choosingDirectory = false"
    @select="chooseDirectory"
  />
  <ConfirmDialog
    :open="!!pendingTemplate"
    title="替换 YAML 内容"
    message="切换模板将替换当前已修改的 YAML。是否继续？"
    confirm-label="替换"
    @close="pendingTemplate = ''"
    @confirm="applyTemplate(pendingTemplate)"
  />
</template>
