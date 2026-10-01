<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, ref, watch } from 'vue'
import { open as dialogOpen, save as dialogSave } from '@tauri-apps/plugin-dialog'
import {
  UiAlert,
  UiCheckbox,
  UiButton,
  UiInput,
  UiModal,
  UiSpinner,
  UiSwitch,
  UiField,
  UiPanel,
} from '@/core/ui'
import { CredentialPicker } from '@/core/vault'
import { connectionIpc } from './ipc'
import type { DriverInstallProgress, DriverStatus } from './contracts'
import {
  isOracleDriverInstallCancelled,
  startOracleDriverInstall,
  type OracleDriverInstallScope,
} from './connection/useOracleDriver'
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
  operationError: [message: string]
}>()

const form = reactive({
  credentialId: '',
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

const clearPassword = ref(false)
let formRevision = 0
const testing = ref(false)
const testResult = ref<{ ok: boolean; message: string } | null>(null)
const saving = ref(false)
const saveError = ref('')
const oracleDriverStatus = ref<DriverStatus | null>(null)
const oracleDriverStatusError = ref('')
const oracleInstallError = ref('')
const oracleInstallCancelled = ref(false)
const oracleInstallProgress = ref<DriverInstallProgress | null>(null)
const oracleInstalling = ref(false)
const driverNote = ref('')
const driverChecking = ref(false)
const driverUnavailable = ref(false)
let driverRevision = 0
let oracleInstallRevision = 0
let activeOracleInstall: OracleDriverInstallScope | undefined
let disposed = false

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function cancelOracleInstall() {
  const install = activeOracleInstall
  activeOracleInstall = undefined
  oracleInstalling.value = false
  oracleInstallProgress.value = null
  install?.cancel()
}

watch(
  () => [props.open, form.dbType] as const,
  async ([open, type]) => {
    const revision = ++driverRevision
    cancelOracleInstall()
    driverNote.value = ''
    driverChecking.value = false
    driverUnavailable.value = false
    oracleDriverStatus.value = null
    oracleDriverStatusError.value = ''
    oracleInstallError.value = ''
    oracleInstallCancelled.value = false
    if (!open || !['oracle', 'kingbase', 'vastbase'].includes(type)) return
    driverChecking.value = true
    try {
      const status = await connectionIpc.driverStatus(type)
      if (revision !== driverRevision) return
      if (type === 'oracle') {
        oracleDriverStatus.value = status
      } else {
        driverUnavailable.value = !status.ready
        driverNote.value = status.ready
          ? '已找到驱动文件；连接时还会校验协议和会话能力。'
          : '尚未安装兼容的 agent 驱动：' +
            (status.dir ?? '') +
            '。需要协议版本 2 及多会话支持，JAR 文件不能直接作为可执行 agent。'
      }
    } catch (error) {
      if (revision === driverRevision) {
        if (type === 'oracle') oracleDriverStatusError.value = errorMessage(error)
        else {
          driverUnavailable.value = true
          driverNote.value = errorMessage(error)
        }
      }
    } finally {
      if (revision === driverRevision) driverChecking.value = false
    }
  },
  { immediate: true }
)

const oracleStatusText = computed(() => {
  if (driverChecking.value) return '检查 Oracle 驱动…'
  if (oracleDriverStatusError.value) return 'Oracle 驱动状态读取失败'
  return oracleDriverStatus.value?.ready ? 'Oracle 驱动已就绪' : 'Oracle 驱动未安装'
})

function oracleProgressText(progress: DriverInstallProgress | null): string {
  if (!progress) return '准备 Oracle 驱动…'
  const labels: Record<DriverInstallProgress['phase'], string> = {
    waiting: '准备 Oracle 驱动…',
    downloading: '下载 Oracle 驱动',
    verifying: '校验 Oracle 驱动…',
    extracting: '解压 Oracle 驱动…',
    validating: '验证 Oracle 驱动…',
    complete: 'Oracle 驱动已就绪',
  }
  if (progress.phase === 'downloading' && progress.totalBytes && progress.totalBytes > 0) {
    const percent = Math.min(
      100,
      Math.floor((progress.downloadedBytes / progress.totalBytes) * 100)
    )
    return `${labels[progress.phase]} ${percent}%`
  }
  return labels[progress.phase]
}

async function prepareOracleDriver(): Promise<DriverStatus | null> {
  if (oracleInstalling.value) return null
  const revision = ++oracleInstallRevision
  oracleInstallError.value = ''
  oracleInstallCancelled.value = false
  oracleInstallProgress.value = null
  oracleInstalling.value = true
  let install: OracleDriverInstallScope | undefined
  try {
    install = startOracleDriverInstall({
      onProgress: (progress) => {
        if (revision !== oracleInstallRevision || !oracleInstalling.value) return
        oracleInstallProgress.value = progress
      },
      onError: (error) => {
        const message = `Oracle 驱动操作失败：${errorMessage(error)}`
        if (revision !== oracleInstallRevision) {
          emit('operationError', message)
          return
        }
        oracleInstallError.value = message
        if (disposed || !props.open) emit('operationError', message)
      },
    })
    activeOracleInstall = install
    const status = await install.promise
    if (revision !== oracleInstallRevision || !props.open) return null
    if (!status.ready) throw new Error(status.note || 'Oracle agent 驱动安装后仍不可用。')
    oracleDriverStatus.value = status
    oracleInstallProgress.value = {
      requestId: install.requestId,
      dbType: 'oracle',
      phase: 'complete',
      downloadedBytes: oracleInstallProgress.value?.downloadedBytes ?? 0,
      ...(oracleInstallProgress.value?.totalBytes === undefined
        ? {}
        : { totalBytes: oracleInstallProgress.value.totalBytes }),
    }
    return status
  } catch (error) {
    if (revision === oracleInstallRevision && !isOracleDriverInstallCancelled(error)) {
      const message = `Oracle 驱动安装失败：${errorMessage(error)}`
      if (props.open) oracleInstallError.value = message
      else emit('operationError', message)
    }
    return null
  } finally {
    if (revision === oracleInstallRevision) {
      if (activeOracleInstall === install) activeOracleInstall = undefined
      oracleInstalling.value = false
    }
  }
}

function stopOracleInstall() {
  if (!activeOracleInstall) return
  cancelOracleInstall()
  oracleInstallCancelled.value = true
}

async function retryOracleStatus() {
  const revision = ++driverRevision
  driverChecking.value = true
  oracleDriverStatusError.value = ''
  try {
    const status = await connectionIpc.driverStatus('oracle')
    if (revision === driverRevision && props.open && form.dbType === 'oracle')
      oracleDriverStatus.value = status
  } catch (error) {
    if (revision === driverRevision && props.open)
      oracleDriverStatusError.value = errorMessage(error)
  } finally {
    if (revision === driverRevision) driverChecking.value = false
  }
}

async function onDownloadOracle() {
  if (oracleDriverStatusError.value) {
    await retryOracleStatus()
    return
  }
  if (oracleDriverStatus.value?.autoInstall === false) return
  await prepareOracleDriver()
}

function closeDialog() {
  formRevision++
  driverRevision++
  cancelOracleInstall()
  emit('close')
}

onBeforeUnmount(() => {
  disposed = true
  formRevision++
  driverRevision++
  cancelOracleInstall()
})
async function pickNewFile() {
  try {
    const path = await dialogSave({
      defaultPath: 'test.sqlite',
      filters: [{ name: 'SQLite', extensions: ['sqlite', 'db'] }],
    })
    if (path) {
      form.host = path
      form.readonly = false
    }
  } catch (error) {
    saveError.value = String(error)
  }
}

watch(
  () => props.open,
  (open) => {
    formRevision++
    if (!open) return
    clearPassword.value = false
    const editing = props.editing
    saveError.value = ''
    testResult.value = null
    if (editing) {
      Object.assign(form, {
        credentialId: editing.credentialId ?? '',
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
        credentialId: '',
        id: `conn-${crypto.randomUUID()}`,
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
  },
  { immediate: true }
)

watch(form, () => {
  formRevision++
  testResult.value = null
  if (oracleInstalling.value) {
    oracleInstallError.value = ''
    oracleInstallCancelled.value = false
    cancelOracleInstall()
  }
})
watch(clearPassword, () => {
  formRevision++
  testResult.value = null
  if (oracleInstalling.value) {
    oracleInstallError.value = ''
    oracleInstallCancelled.value = false
    cancelOracleInstall()
  }
})
watch(
  () => form.env,
  (value) => {
    if (['生产', 'production', 'prod'].includes(value.trim().toLowerCase())) form.readonly = true
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
    credentialId: clearPassword.value || fileType.value ? null : form.credentialId || null,
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
  if (testing.value) return
  const revision = formRevision
  const config = buildConfig()
  const password = form.password
  const shouldClearPassword = clearPassword.value
  let oracleConnectionTestStarted = false
  testing.value = true
  testResult.value = null
  try {
    if (config.dbType === 'oracle') {
      oracleDriverStatusError.value = ''
      const status = await connectionIpc.driverStatus('oracle')
      if (disposed || revision !== formRevision || !props.open) return
      oracleDriverStatus.value = status
      if (!status.ready) {
        if (status.autoInstall === false) {
          oracleInstallError.value = status.note || 'Oracle agent 驱动不可自动安装。'
          return
        }
        const installed = await prepareOracleDriver()
        if (!installed || revision !== formRevision || !props.open) return
      }
    }
    if (disposed || revision !== formRevision || !props.open) return
    oracleConnectionTestStarted = config.dbType === 'oracle'
    const version = await withTimeout(
      connectionIpc.test(config, password, shouldClearPassword),
      Math.max(1000, Math.min(config.connectTimeoutMs, 120000)) + 5000,
      '测试连接超时：请检查网络与服务器配置'
    )
    if (!disposed && revision === formRevision && props.open) {
      testResult.value = { ok: true, message: `连接成功 · ${version}` }
    }
  } catch (err) {
    if (!disposed && revision === formRevision && props.open) {
      const message = errorMessage(err)
      if (config.dbType === 'oracle' && !oracleConnectionTestStarted)
        oracleDriverStatusError.value = message
      else testResult.value = { ok: false, message }
    }
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
    const password = form.password
    await connectionIpc.save(config, password, clearPassword.value)
    emit('saved', config, password)
    closeDialog()
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
    placement="top"
    @close="closeDialog"
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
            ><UiInput v-model.number="form.port" size="xs" type="number" class="connection-port"
          /></UiField>
        </div>
        <UiField
          label="凭证"
          size="xs"
          :description="
            form.credentialId ? undefined : '选择公共凭证，或在下方输入；保存后密码统一进入凭证库。'
          "
        >
          <CredentialPicker
            v-model="form.credentialId"
            :kind="form.dbType === 'redis' ? undefined : 'password'"
            size="xs"
            :disabled="clearPassword"
          />
        </UiField>
        <div v-if="!form.credentialId || clearPassword" class="grid grid-cols-2 gap-[8px]">
          <UiField label="用户名" size="xs"
            ><UiInput v-model="form.username" size="xs" placeholder="例如：db_user"
          /></UiField>
          <UiField
            label="密码"
            size="xs"
            :description="editing && !clearPassword ? '留空保留已保存的密码' : undefined"
            ><UiInput v-model="form.password" size="xs" type="password" :disabled="clearPassword"
          /></UiField>
        </div>
        <UiCheckbox
          v-if="editing"
          v-model="clearPassword"
          size="xs"
          label="清空已保存密码，以无密码方式连接"
        />
        <UiField v-if="form.dbType !== 'redis'" label="默认数据库" size="xs">
          <UiInput v-model="form.database" size="xs" placeholder="例如：app_data" />
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
          <UiButton size="xs" variant="ghost" @click="pickNewFile">新建路径</UiButton>
        </div>
      </UiField>
      <p v-if="fileType" class="text-caption text-text-muted dark:text-text-muted-dark">
        保存后连接时可创建新 SQLite 文件；测试连接只打开已有文件。
      </p>
      <div
        v-if="form.dbType === 'oracle'"
        class="flex h-[22px] min-w-0 items-center gap-[8px] overflow-hidden whitespace-nowrap text-caption"
        aria-live="polite"
      >
        <span class="shrink-0 text-secondary dark:text-secondary-dark">
          {{ oracleInstalling ? oracleProgressText(oracleInstallProgress) : oracleStatusText }}
        </span>
        <span
          v-if="oracleInstallError || oracleDriverStatusError || oracleInstallCancelled"
          class="min-w-0 truncate"
          :class="
            oracleInstallError || oracleDriverStatusError
              ? 'text-danger-strong dark:text-danger-dark'
              : 'text-secondary dark:text-secondary-dark'
          "
          :title="oracleInstallError || oracleDriverStatusError || undefined"
        >
          {{
            oracleInstallError ||
            (oracleDriverStatusError
              ? `读取 Oracle 驱动状态失败：${oracleDriverStatusError}`
              : oracleInstallCancelled
                ? 'Oracle 驱动下载已取消'
                : '')
          }}
        </span>
        <UiButton
          v-if="oracleInstalling"
          size="xs"
          variant="ghost"
          :disabled="saving"
          @click="stopOracleInstall"
        >
          停止
        </UiButton>
        <UiButton
          v-else-if="oracleDriverStatusError"
          size="xs"
          variant="ghost"
          :disabled="driverChecking || saving"
          @click="onDownloadOracle"
        >
          重试
        </UiButton>
        <UiButton
          v-else-if="
            (oracleInstallError || oracleInstallCancelled) &&
            oracleDriverStatus?.autoInstall !== false
          "
          size="xs"
          variant="ghost"
          :disabled="saving"
          @click="onDownloadOracle"
        >
          重试
        </UiButton>
        <UiButton
          v-else-if="
            oracleDriverStatus?.ready === false && oracleDriverStatus.autoInstall !== false
          "
          size="xs"
          variant="ghost"
          :disabled="driverChecking || saving"
          @click="onDownloadOracle"
        >
          下载
        </UiButton>
      </div>
      <UiAlert v-else-if="driverChecking || driverNote" tone="info" title="驱动状态" size="sm">
        {{ driverChecking ? '检查驱动…' : driverNote }}
      </UiAlert>
      <UiPanel
        :key="String(open) + form.id"
        title="更多配置"
        padding="none"
        class="connection-more"
        collapsible
        :default-open="false"
      >
        <template #header>
          <span class="text-caption font-medium text-secondary dark:text-secondary-dark"
            >更多配置</span
          >
        </template>
        <div class="space-y-[8px] pt-[4px]">
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
      <UiButton size="xs" variant="ghost" :disabled="saving" @click="closeDialog">取消</UiButton>
      <UiButton
        size="xs"
        variant="secondary"
        :disabled="
          testing ||
          saving ||
          (form.dbType === 'oracle'
            ? driverChecking || oracleInstalling
            : driverChecking || driverUnavailable) ||
          unsupported
        "
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

<style scoped>
.connection-more {
  border: 0;
  border-radius: 0;
  background: transparent;
}
.connection-more :deep(header) {
  min-height: 24px;
  margin-bottom: 0;
}
.connection-port {
  appearance: textfield;
}
.connection-port::-webkit-inner-spin-button,
.connection-port::-webkit-outer-spin-button {
  appearance: none;
  margin: 0;
}
</style>
