<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { open as dialogOpen } from '@tauri-apps/plugin-dialog'
import {
  UiAlert,
  UiButton,
  UiInput,
  UiModal,
  UiSpinner,
  UiSwitch,
  UiField,
  UiPanel,
} from '@/core/ui'
import { connectionIpc } from './ipc'
import { withTimeout } from './useDatabase'
import {
  DB_TYPE_META,
  DEFAULT_PORT,
  defaultDatabaseFor,
  isFileType,
  isUnsupportedType,
  type V2DbType,
} from './useDatabaseMeta'
import ConnectionBasicsFields from './ConnectionBasicsFields.vue'

const props = defineProps<{
  open: boolean
  editing?: import('./contracts').ConnConfig | null
}>()

const emit = defineEmits<{
  close: []
  saved: [config: import('./contracts').ConnConfig, password: string]
  tested: [ok: boolean, message: string]
}>()

const form = reactive({
  id: '',
  label: '',
  dbType: 'postgresql' as V2DbType,
  host: '',
  port: 0,
  username: '',
  password: '',
  database: '',
  env: '开发',
  readonly: false,
  ssl: false,
  connectTimeoutMs: 30000,
})

const testing = ref(false)
const testResult = ref<{ ok: boolean; message: string } | null>(null)
const saving = ref(false)
const saveError = ref('')

watch(
  () => props.open,
  (open) => {
    if (!open) return
    const editing = props.editing
    saveError.value = ''
    testResult.value = null
    if (editing) {
      Object.assign(form, {
        id: editing.id,
        label: editing.label,
        dbType: editing.dbType,
        host: editing.host,
        port: editing.port,
        username: editing.username,
        password: '',
        database: editing.database,
        env: editing.env,
        readonly: editing.readonly,
        ssl: editing.ssl,
        connectTimeoutMs: editing.connectTimeoutMs,
      })
    } else {
      Object.assign(form, {
        id: `conn-${Date.now()}`,
        label: '',
        dbType: 'postgresql',
        host: '127.0.0.1',
        port: DEFAULT_PORT.postgresql,
        username: '',
        password: '',
        database: defaultDatabaseFor('postgresql'),
        env: '开发',
        readonly: false,
        ssl: false,
        connectTimeoutMs: 30000,
      })
    }
  }
)

function pickType(type: V2DbType) {
  form.dbType = type
  // 切换类型时按类型默认值刷新端口/库名（sqlite 无端口）
  form.port = DEFAULT_PORT[type]
  form.database = defaultDatabaseFor(type)
}

const fileType = computed(() => isFileType(form.dbType))
const unsupported = computed(() => isUnsupportedType(form.dbType))

async function pickFile() {
  try {
    const picked = await dialogOpen({
      multiple: false,
      filters: [{ name: 'SQLite', extensions: ['db', 'sqlite', 'sqlite3'] }],
    })
    if (picked) form.host = picked
  } catch (err) {
    saveError.value = String(err)
  }
}

function buildConfig(): import('./contracts').ConnConfig {
  return {
    id: form.id,
    label: form.label.trim() || `${DB_TYPE_META[form.dbType].label} 连接`,
    dbType: form.dbType,
    host: form.host.trim(),
    port: form.port,
    username: form.username.trim(),
    database: form.database.trim() || defaultDatabaseFor(form.dbType),
    env: form.env,
    readonly: form.readonly,
    ssl: form.ssl,
    connectTimeoutMs: form.connectTimeoutMs,
  }
}

async function onTest() {
  if (unsupported.value) {
    testResult.value = { ok: false, message: '达梦驱动暂未支持（本版本未实现）。' }
    return
  }
  testing.value = true
  testResult.value = null
  try {
    const version = await withTimeout(
      connectionIpc.test(buildConfig(), form.password),
      30000,
      '测试连接超时（30 秒）：请检查网络与服务器配置'
    )
    testResult.value = { ok: true, message: `连接成功 · ${version}` }
  } catch (err) {
    testResult.value = { ok: false, message: String(err) }
  } finally {
    testing.value = false
  }
}

async function onSave() {
  if (unsupported.value) {
    saveError.value = '达梦驱动暂未支持（本版本未实现），无法保存连接。'
    return
  }
  saving.value = true
  saveError.value = ''
  try {
    // buildConfig 只取一次：保存与回传必须是同一份表单快照
    const config = buildConfig()
    await connectionIpc.save(config, form.password)
    emit('saved', config, form.password)
    emit('close')
  } catch (err) {
    saveError.value = String(err)
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <UiModal
    :open="open"
    :title="editing ? '编辑连接' : '新建连接'"
    width="min(460px, 92vw)"
    @close="emit('close')"
  >
    <div class="space-y-[8px]">
      <UiAlert v-if="unsupported" tone="warning" title="暂未支持" size="sm"
        >达梦驱动本版本未实现，请选择其它数据库类型。</UiAlert
      >
      <ConnectionBasicsFields
        :label="form.label"
        :db-type="form.dbType"
        @update:label="form.label = $event"
        @update:db-type="pickType"
      />
      <template v-if="!fileType">
        <div class="grid grid-cols-[1fr_80px] gap-[8px]">
          <UiField label="主机" size="xs" required
            ><UiInput v-model="form.host" size="xs" placeholder="127.0.0.1"
          /></UiField>
          <UiField label="端口" size="xs" required
            ><UiInput v-model.number="form.port" size="xs" type="number"
          /></UiField>
        </div>
        <div class="grid grid-cols-2 gap-[8px]">
          <UiField label="用户名" size="xs"
            ><UiInput v-model="form.username" size="xs" placeholder="例如：db_user"
          /></UiField>
          <UiField
            label="密码"
            size="xs"
            :description="editing ? '留空保留已保存的密码' : undefined"
            ><UiInput v-model="form.password" size="xs" type="password"
          /></UiField>
        </div>
        <UiField :label="form.dbType === 'redis' ? '数据库索引' : '默认数据库'" size="xs">
          <UiInput
            v-model="form.database"
            size="xs"
            :placeholder="form.dbType === 'redis' ? 'db0' : '例如：app_data'"
          />
        </UiField>
      </template>
      <UiField
        v-else
        label="SQLite 文件路径"
        size="xs"
        required
        description="文件不存在时将创建数据库。"
      >
        <div class="flex gap-[4px]">
          <UiInput
            v-model="form.host"
            size="xs"
            placeholder="C:\data\app.db"
            class="min-w-0 flex-1"
          />
          <UiButton size="xs" variant="secondary" @click="pickFile">选择文件</UiButton>
        </div>
      </UiField>
      <UiPanel
        :key="String(open) + form.id"
        title="更多配置"
        padding="xs"
        collapsible
        :default-open="false"
      >
        <div class="space-y-[8px]">
          <div class="grid grid-cols-2 gap-[8px]">
            <UiField label="环境" size="xs"
              ><UiInput v-model="form.env" size="xs" placeholder="开发"
            /></UiField>
            <UiField v-if="!fileType" label="连接超时（毫秒）" size="xs"
              ><UiInput v-model.number="form.connectTimeoutMs" size="xs" type="number"
            /></UiField>
          </div>
          <div class="flex items-center gap-[12px]">
            <UiField v-if="!fileType" label="SSL" size="xs"
              ><UiSwitch v-model="form.ssl" size="sm"
            /></UiField>
            <UiField label="只读" size="xs"><UiSwitch v-model="form.readonly" size="sm" /></UiField>
          </div>
        </div>
      </UiPanel>
      <UiAlert v-if="saveError" tone="danger" title="保存失败" size="sm">{{ saveError }}</UiAlert>
      <UiAlert
        v-if="testResult"
        :tone="testResult.ok ? 'success' : 'danger'"
        :title="testResult.ok ? '连接成功' : '连接失败'"
        size="sm"
        >{{ testResult.message }}</UiAlert
      >
    </div>
    <template #footer>
      <UiButton size="xs" variant="ghost" :disabled="saving" @click="emit('close')">取消</UiButton>
      <UiButton
        size="xs"
        variant="secondary"
        :disabled="testing || saving"
        :title="editing && !form.password ? '使用已保存密码测试' : undefined"
        @click="onTest"
        ><UiSpinner v-if="testing" size="xs" label="测试中" /><template v-else
          >测试连接</template
        ></UiButton
      >
      <UiButton size="xs" variant="primary" :disabled="saving || testing" @click="onSave"
        ><UiSpinner v-if="saving" size="xs" label="保存中" /><template v-else
          >保存</template
        ></UiButton
      >
    </template>
  </UiModal>
</template>
