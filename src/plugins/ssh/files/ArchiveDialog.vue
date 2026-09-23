<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { UiModal, UiField, UiInput, UiSelect, UiButton, UiAlert } from '@/core/ui'
import type { ArchiveRequest, RemoteFile } from '../contracts'
import { archiveFormat, archiveOutputName } from './archiveFiles'
const props = defineProps<{
  mode: 'compress' | 'extract'
  files: RemoteFile[]
  directory: string
}>()
const emit = defineEmits<{ close: []; submit: [request: ArchiveRequest] }>()
const format = ref<ArchiveRequest['format']>(
  props.mode === 'extract' ? (archiveFormat(props.files[0].path) ?? 'zip') : 'zip'
)
const folder = ref(props.directory)
const basename = props.files.length === 1 ? props.files[0].name : '归档'
const filename = ref(props.mode === 'extract' ? archiveOutputName(basename) : basename + '.zip')
const error = ref('')
const options = computed(() => [
  { value: 'zip', label: 'ZIP' },
  { value: 'tar.gz', label: 'TAR.GZ' },
  ...(props.files.length === 1 && !props.files[0].isDir
    ? [{ value: 'gz', label: 'GZ · 单文件' }]
    : []),
])
watch(format, (value, old) => {
  if (props.mode === 'compress')
    filename.value = filename.value.endsWith('.' + old)
      ? filename.value.slice(0, -(old.length + 1)) + '.' + value
      : filename.value + '.' + value
})
function submit() {
  const name = filename.value.trim(),
    parent = folder.value.trim()
  if (!parent.startsWith('/') || [...parent].some((char) => char.charCodeAt(0) < 32)) {
    error.value = '请输入远程目录绝对路径'
    return
  }
  if (
    !name ||
    ['.', '..'].includes(name) ||
    name.includes('/') ||
    name.includes(String.fromCharCode(92)) ||
    [...name].some((char) => char.charCodeAt(0) < 32)
  ) {
    error.value = '名称不能包含路径分隔符'
    return
  }
  emit('submit', {
    operation: props.mode,
    format: format.value,
    paths: props.files.map((file) => file.path),
    output: parent.replace(/\/+$/, '') + '/' + name,
  })
}
</script>
<template>
  <UiModal open :title="mode === 'compress' ? '压缩文件' : '解压到服务器'" @close="emit('close')">
    <div class="space-y-md">
      <p class="truncate select-text text-body-sm">
        {{ files.map((file) => file.name).join('、') }}
      </p>
      <UiField v-if="mode === 'compress'" label="格式"
        ><UiSelect v-model="format" :options="options"
      /></UiField>
      <UiField label="远程目标目录"><UiInput v-model="folder" /></UiField>
      <UiField :label="mode === 'extract' && format !== 'gz' ? '新目录名称' : '文件名称'"
        ><UiInput v-model="filename"
      /></UiField>
      <p class="text-caption text-secondary dark:text-secondary-dark">
        源文件保留；目标已存在时不覆盖。任务开始后可在右下角传输浮层查看或取消。
      </p>
      <UiAlert v-if="error" tone="danger">{{ error }}</UiAlert>
    </div>
    <template #footer
      ><UiButton variant="ghost" @click="emit('close')">取消</UiButton
      ><UiButton @click="submit">{{
        mode === 'compress' ? '开始压缩' : '开始解压'
      }}</UiButton></template
    >
  </UiModal>
</template>
