<script setup lang="ts">
import { UiScrollArea, UiCodeEditor } from '@/core/ui'
/**
 * 新建数据库对话框（分类型表单，对齐 dbx）：
 * - mysql/polardb：库名 + 字符集/排序规则联动 + 用户授权（权限白名单）+ SQL 预览 + 分步结果
 * - PG 系：库名 + 编码（静态清单）
 * 字符集/用户清单打开时从后端拉取，失败/为空时用兜底清单。
 */
import { computed, ref, watch } from 'vue'
import { UiButton, UiCheckbox, UiIcon, UiInput, UiModal, UiSelect, UiSpinner } from '@/core/ui'
import { FALLBACK_CHARSETS, FALLBACK_COLLATIONS, type V2DbType } from './useDatabaseMeta'
import { adminIpc } from './ipc'
import type { DbConnectionInfo, DbGrantInput, DbStepResult, DbUserInfo } from './contracts'
import type { useDatabase } from './useDatabase'

const props = defineProps<{
  db: ReturnType<typeof useDatabase>
  /** 目标连接（null = 关闭） */
  connection: DbConnectionInfo | null
}>()

const emit = defineEmits<{ (e: 'close'): void }>()

const { db } = props

// ── 表单状态 ──
const name = ref('')
const charset = ref('utf8mb4')
const collation = ref('utf8mb4_unicode_ci')
/** 授权：选中的 user@host → 权限 */
const grants = ref<Record<string, DbGrantInput['privilege']>>({})
const privilegePreset = ref<DbGrantInput['privilege']>('readwrite')

// ── 远端选项 ──
const loading = ref(false)
const charsetOptions = ref<string[]>([...FALLBACK_CHARSETS])
const collationsByCharset = ref<Record<string, string[]>>({ ...FALLBACK_COLLATIONS })
const users = ref<DbUserInfo[]>([])
const steps = ref<DbStepResult[] | null>(null)
const submitting = ref(false)

/** 是否 mysql 系（有字符集/授权）；pg 系只有名称 + 编码 */
const isMysql = computed(() => {
  const t = props.connection?.dbType as V2DbType | undefined
  return t === 'mysql' || t === 'polardb'
})

const PG_ENCODINGS = ['UTF8', 'LATIN1', 'SQL_ASCII', 'WIN1252']

const charsetSelectOptions = computed(() => {
  if (isMysql.value) return charsetOptions.value.map((v) => ({ value: v }))
  return PG_ENCODINGS.map((v) => ({ value: v }))
})

const collationOptions = computed(() =>
  (collationsByCharset.value[charset.value] ?? []).map((v) => ({ value: v }))
)

/** 字符集切换 → 排序规则联动到该字符集首个 */
watch(charset, (v) => {
  if (!isMysql.value) return
  collation.value = collationsByCharset.value[v]?.[0] ?? ''
})

/** 打开时拉取字符集与用户清单 */
watch(
  () => props.connection,
  async (conn) => {
    if (!conn) return
    name.value = ''
    steps.value = null
    grants.value = {}
    charset.value = isMysql.value ? 'utf8mb4' : 'UTF8'
    collation.value = 'utf8mb4_unicode_ci'
    if (!isMysql.value) return
    loading.value = true
    try {
      const options = await adminIpc.charsetOptions(conn.id)
      if (options.charsets.length) {
        charsetOptions.value = options.charsets
        collationsByCharset.value = options.collationsByCharset
        collation.value = options.collationsByCharset[charset.value]?.[0] ?? ''
      }
      users.value = await adminIpc.users(conn.id)
    } catch {
      // 拉取失败保留兜底清单，不阻塞建库
    } finally {
      loading.value = false
    }
  },
  { immediate: true }
)

/** SQL 预览（与后端方言构造逻辑一致，仅作展示） */
const previewSql = computed(() => {
  const n = name.value.trim()
  if (!n) return ''
  const quote = isMysql.value ? `\`${n}\`` : `"${n}"`
  let sql = `CREATE DATABASE ${quote}`
  if (isMysql.value) {
    if (charset.value) sql += ` DEFAULT CHARACTER SET ${charset.value}`
    if (collation.value) sql += ` DEFAULT COLLATE ${collation.value}`
  } else if (charset.value) {
    sql += ` ENCODING '${charset.value}'`
  }
  const grantList = selectedGrants.value
  for (const g of grantList) {
    const priv =
      g.privilege === 'all'
        ? 'ALL PRIVILEGES'
        : g.privilege === 'readwrite'
          ? 'SELECT, INSERT, UPDATE, DELETE'
          : 'SELECT'
    sql += `;\nGRANT ${priv} ON \`${n}\`.* TO '${g.user}'@'${g.host}'`
  }
  return sql
})

const selectedGrants = computed<DbGrantInput[]>(() =>
  Object.entries(grants.value).map(([key, privilege]) => {
    const [user, host] = key.split('@')
    return { user, host, privilege }
  })
)

const canSubmit = computed(() => name.value.trim().length > 0 && !submitting.value)

function toggleUser(u: DbUserInfo, checked: boolean) {
  const key = `${u.user}@${u.host}`
  if (checked) grants.value[key] = privilegePreset.value
  else delete grants.value[key]
}

async function submit() {
  const conn = props.connection
  if (!conn || !canSubmit.value) return
  submitting.value = true
  steps.value = null
  try {
    const result = await db.createDatabaseFull(
      conn.id,
      name.value,
      charset.value || undefined,
      isMysql.value ? collation.value || undefined : undefined,
      selectedGrants.value.length ? selectedGrants.value : undefined
    )
    if (!result) return
    steps.value = result
    if (result.every((s) => s.ok)) {
      // 全部成功：短暂展示后关闭
      setTimeout(() => emit('close'), 800)
    }
  } finally {
    submitting.value = false
  }
}
</script>

<template>
  <UiModal
    :open="connection !== null"
    :title="`新建数据库 · ${connection?.label ?? ''}`"
    size="md"
    @close="emit('close')"
  >
    <div class="space-y-[8px]">
      <div>
        <p class="mb-[4px] text-body-sm text-secondary dark:text-secondary-dark">数据库名</p>
        <UiInput
          v-model="name"
          size="xs"
          placeholder="字母或下划线开头，可含数字/下划线/$"
          @keydown.enter="submit"
        />
      </div>

      <div class="flex gap-[8px]">
        <div class="flex-1">
          <p class="mb-[4px] text-body-sm text-secondary dark:text-secondary-dark">
            {{ isMysql ? '字符集' : '编码' }}
          </p>
          <UiSelect v-model="charset" :options="charsetSelectOptions" size="xs" class="w-full" />
        </div>
        <div v-if="isMysql" class="flex-1">
          <p class="mb-[4px] text-body-sm text-secondary dark:text-secondary-dark">排序规则</p>
          <UiSelect v-model="collation" :options="collationOptions" size="xs" class="w-full" />
        </div>
      </div>

      <!-- 授权（仅 mysql 系） -->
      <div v-if="isMysql">
        <div class="mb-[4px] flex items-center justify-between">
          <p class="text-body-sm text-secondary dark:text-secondary-dark">授权用户（可选）</p>
          <UiSelect
            v-model="privilegePreset"
            :options="[
              { value: 'readonly', label: '只读（SELECT）' },
              { value: 'readwrite', label: '读写（增删改查）' },
              { value: 'all', label: '全部权限' },
            ]"
            size="xs"
            class="w-[170px]"
          />
        </div>
        <UiScrollArea as-child axis="vertical">
          <div
            class="max-h-[140px] rounded-[6px] border border-border p-[6px] dark:border-border-dark"
          >
            <UiSpinner v-if="loading" size="xs" class="mx-auto my-[8px]" />
            <p
              v-else-if="!users.length"
              class="py-[8px] text-center text-caption text-text-muted dark:text-text-muted-dark"
            >
              无可用用户（或查询失败）
            </p>
            <label
              v-for="u in users"
              :key="`${u.user}@${u.host}`"
              class="flex cursor-pointer items-center gap-[6px] rounded-[4px] px-[6px] py-[3px] text-body-sm hover:bg-border dark:hover:bg-border-dark"
            >
              <UiCheckbox
                :model-value="`${u.user}@${u.host}` in grants"
                size="xs"
                @update:model-value="(checked) => toggleUser(u, checked)"
              />
              <span class="font-mono text-caption">{{ u.user }}@{{ u.host }}</span>
            </label>
          </div>
        </UiScrollArea>
        <p
          v-if="Object.keys(grants).length"
          class="mt-[4px] text-caption text-text-muted dark:text-text-muted-dark"
        >
          已选 {{ Object.keys(grants).length }} 个用户，建库后逐个授权
        </p>
      </div>

      <!-- SQL 预览 -->
      <div v-if="previewSql">
        <p class="mb-[4px] text-body-sm text-secondary dark:text-secondary-dark">SQL 预览</p>
        <UiCodeEditor
          :model-value="previewSql"
          language="sql"
          readonly
          :completion="false"
          height="100px"
        />
      </div>

      <!-- 分步执行结果 -->
      <div v-if="steps" class="space-y-[4px]">
        <div v-for="(s, i) in steps" :key="i" class="flex items-start gap-[6px] text-body-sm">
          <UiIcon
            :name="s.ok ? 'check' : 'x'"
            :size="12"
            :stroke-width="2.5"
            class="mt-[3px] shrink-0"
            :class="
              s.ok
                ? 'text-success-strong dark:text-success-dark'
                : 'text-danger-strong dark:text-danger-dark'
            "
          />
          <div class="min-w-0">
            <span>{{ s.label }}</span>
            <p v-if="s.error" class="text-caption text-danger-strong dark:text-danger-dark">
              {{ s.error }}
            </p>
          </div>
        </div>
      </div>
    </div>

    <template #footer>
      <UiButton size="xs" variant="ghost" @click="emit('close')">取消</UiButton>
      <UiButton size="xs" variant="primary" :disabled="!canSubmit" @click="submit">
        {{ submitting ? '创建中…' : '创建' }}
      </UiButton>
    </template>
  </UiModal>
</template>
