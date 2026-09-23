<script setup lang="ts">
import { computed, ref, onUnmounted } from 'vue'
import { save } from '@tauri-apps/plugin-dialog'
import {
  UiModal,
  UiButton,
  UiSelect,
  UiCheckbox,
  UiAlert,
  UiField,
  UiScrollArea,
  UiSearchInput,
} from '@/core/ui'
import { ipc } from '../ipc'
import { useToolLifecycle } from '@/core/lifecycle'
import type { ServerProfile, SshGroup } from '../contracts'
const props = defineProps<{ profiles: ServerProfile[]; groups: SshGroup[]; groupId?: string }>()
const emit = defineEmits<{ close: [] }>()
const scope = ref(props.groupId ?? 'all'),
  query = ref(''),
  selected = ref(new Set<string>()),
  password = ref(false),
  busy = ref(false),
  error = ref(''),
  done = ref('')
let disposed = false
onUnmounted(() => {
  disposed = true
})
useToolLifecycle('ssh', {
  owner: 'ssh.export',
  prepare: () => (busy.value ? '服务器正在导出' : null),
})
const options = computed(() => [
  { value: 'all', label: '全部服务器' },
  { value: 'selected', label: '手动选择' },
  ...props.groups.map((g) => ({ value: g.id, label: g.name })),
])
const visible = computed(() =>
  props.profiles.filter((p) =>
    (p.name + ' ' + p.host).toLowerCase().includes(query.value.toLowerCase())
  )
)
const ids = computed(() =>
  props.profiles
    .filter(
      (p) =>
        scope.value === 'all' ||
        (scope.value === 'selected' ? selected.value.has(p.id) : p.groupId === scope.value)
    )
    .map((p) => p.id)
)
async function run() {
  if (busy.value || !ids.value.length) return
  busy.value = true
  error.value = ''
  done.value = ''
  try {
    const path = await save({
      defaultPath: 'servers.csv',
      filters: [{ name: 'CSV', extensions: ['csv'] }],
    })
    if (path && !disposed) {
      const count = await ipc.sshServerCsvExport({
        path,
        ids: [...ids.value],
        includePassword: password.value,
      })
      done.value = '已导出 ' + count + ' 台服务器'
    }
  } catch {
    error.value = '导出失败，请检查文件权限，或刷新服务器列表后重试'
  } finally {
    busy.value = false
  }
}
</script>
<template>
  <UiModal open title="导出服务器" @close="!busy && emit('close')">
    <div class="flex flex-col gap-md">
      <UiField label="导出范围"
        ><UiSelect v-model="scope" :options="options" :disabled="busy"
      /></UiField>
      <template v-if="scope === 'selected'"
        ><UiSearchInput v-model="query" placeholder="筛选服务器" /><UiScrollArea
          class="max-h-[240px]"
          ><div class="flex flex-col gap-sm">
            <UiCheckbox
              v-for="p in visible"
              :key="p.id"
              :label="p.name + ' · ' + p.username + '@' + p.host"
              :model-value="selected.has(p.id)"
              :disabled="busy"
              @update:model-value="$event ? selected.add(p.id) : selected.delete(p.id)"
            /></div></UiScrollArea
      ></template>
      <UiCheckbox v-model="password" label="包含本地保存的密码" :disabled="busy" />
      <UiAlert :tone="password ? 'warning' : 'info'"
        >{{
          password ? 'CSV 将包含本地明文密码，请妥善保管。' : '默认仅导出服务器配置，密码列留空。'
        }}
        凭证库密码及私钥始终不导出，文件内会注明认证类型与密码来源。CSV
        可用于重新导入密码认证服务器。</UiAlert
      >
      <p class="text-caption text-secondary dark:text-secondary-dark">
        已选择 {{ ids.length }} 台服务器。含公式形式的单元格建议用文本编辑器查看。
      </p>
      <UiAlert v-if="error" tone="danger">{{ error }}</UiAlert
      ><UiAlert v-if="done" tone="success">{{ done }}</UiAlert>
    </div>
    <template #footer
      ><UiButton :loading="busy" :disabled="!ids.length" @click="run">导出 CSV</UiButton
      ><UiButton variant="ghost" :disabled="busy" @click="emit('close')">关闭</UiButton></template
    >
  </UiModal>
</template>
