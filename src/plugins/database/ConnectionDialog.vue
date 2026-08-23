<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { open as dialogOpen } from '@tauri-apps/plugin-dialog'
import { UiAlert, UiButton, UiInput, UiModal, UiSpinner, UiSwitch } from '@/core/ui'
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
  <UiModal :open="open" :title="editing ? '编辑连接' : '新建连接'" size="lg" @close="emit('close')">
    <div class="space-y-[12px]">
      <UiAlert v-if="unsupported" tone="warning" title="暂未支持" size="sm">
        达梦（DM8）驱动本版本未实现，可先选择其它数据库类型。
      </UiAlert>

      <ConnectionBasicsFields
        :label="form.label"
        :env="form.env"
        :db-type="form.dbType"
        @update:label="form.label = $event"
        @update:env="form.env = $event"
        @update:db-type="pickType"
      />

      <template v-if="!fileType">
        <div class="grid grid-cols-[1fr_96px] gap-[8px]">
          <div>
            <label
              class="mb-[4px] block text-caption font-medium text-text-muted dark:text-text-muted-dark"
              >主机</label
            >
            <UiInput v-model="form.host" size="sm" placeholder="127.0.0.1" />
          </div>
          <div>
            <label
              class="mb-[4px] block text-caption font-medium text-text-muted dark:text-text-muted-dark"
              >端口</label
            >
            <UiInput v-model.number="form.port" size="sm" type="number" />
          </div>
        </div>
        <div class="grid grid-cols-2 gap-[8px]">
          <div>
            <label
              class="mb-[4px] block text-caption font-medium text-text-muted dark:text-text-muted-dark"
              >用户名</label
            >
            <UiInput v-model="form.username" size="sm" placeholder="patchy" />
          </div>
          <div>
            <label
              class="mb-[4px] block text-caption font-medium text-text-muted dark:text-text-muted-dark"
              >密码</label
            >
            <UiInput
              v-model="form.password"
              size="sm"
              type="password"
              :placeholder="editing ? '留空表示不修改密码' : '••••••'"
            />
          </div>
        </div>
        <div class="grid grid-cols-2 gap-[8px]">
          <div>
            <label
              class="mb-[4px] block text-caption font-medium text-text-muted dark:text-text-muted-dark"
            >
              {{ form.dbType === 'redis' ? '数据库索引（db0/db1…）' : '默认数据库' }}
            </label>
            <UiInput
              v-model="form.database"
              size="sm"
              :placeholder="form.dbType === 'redis' ? 'db0' : 'patchybox'"
            />
          </div>
          <div class="flex items-end gap-[16px] pb-[6px]">
            <label
              class="flex items-center gap-[6px] text-caption text-text-muted dark:text-text-muted-dark"
            >
              <UiSwitch v-model="form.ssl" size="sm" /> SSL
            </label>
            <label
              class="flex items-center gap-[6px] text-caption text-text-muted dark:text-text-muted-dark"
            >
              <UiSwitch v-model="form.readonly" size="sm" /> 只读
            </label>
          </div>
        </div>
      </template>

      <div v-else>
        <label
          class="mb-[4px] block text-caption font-medium text-text-muted dark:text-text-muted-dark"
          >SQLite 文件路径</label
        >
        <div class="flex gap-[8px]">
          <UiInput
            v-model="form.host"
            size="sm"
            placeholder="C:\data\app.db（不存在自动创建）"
            class="flex-1"
          />
          <UiButton
            size="sm"
            variant="secondary"
            @click="
              (async () => {
                const picked = await dialogOpen({
                  multiple: false,
                  filters: [{ name: 'SQLite', extensions: ['db', 'sqlite', 'sqlite3'] }],
                })
                if (picked) form.host = picked
              })()
            "
          >
            选择文件
          </UiButton>
        </div>
      </div>

      <UiAlert v-if="saveError" tone="danger" title="保存失败" size="sm">{{ saveError }}</UiAlert>
      <div v-if="testResult" class="flex items-center gap-[6px]">
        <span
          class="h-[8px] w-[8px] rounded-full"
          :class="testResult.ok ? 'bg-success-strong' : 'bg-danger-strong'"
        />
        <span class="text-caption text-secondary dark:text-secondary-dark">{{
          testResult.message
        }}</span>
      </div>
    </div>

    <template #footer>
      <UiButton size="sm" variant="ghost" :disabled="saving" @click="emit('close')">取消</UiButton>
      <UiButton
        size="sm"
        variant="secondary"
        :disabled="testing || saving"
        :title="editing && !form.password ? '密码留空时使用已保存密码测试' : undefined"
        @click="onTest"
      >
        <UiSpinner v-if="testing" size="xs" label="测试中" />
        <template v-else>测试连接</template>
      </UiButton>
      <UiButton size="sm" variant="primary" :disabled="saving || testing" @click="onSave()">
        <UiSpinner v-if="saving" size="xs" label="保存中" />
        <template v-else>保存</template>
      </UiButton>
    </template>
  </UiModal>
</template>
