<script setup lang="ts">
/**
 * 云解析记录管理 · 记录表格 + 行内添加/编辑表单 + 分页
 * 删除采用两段式确认（再次点击执行，3s 后复原），避免弹窗打断。
 */
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { ipc } from './ipc'
import { useUiStore } from '@/stores/ui'
import type { CloudDomain, CloudRecord } from './contracts'
import { CLOUD_RECORD_TYPES, TTL_PRESETS, recordTypeBadgeClass } from './useDns'
import { UiSelect as Select } from '@/core/ui'

const props = defineProps<{
  platform: 'aliyun' | 'dnspod'
  domain: CloudDomain
}>()

const emit = defineEmits<{ back: [] }>()

const ui = useUiStore()

const records = ref<CloudRecord[]>([])
const total = ref(0)
const page = ref(1)
const pageSize = 50
const busy = ref(false)
/** 记录搜索词（服务端模糊搜索：主机记录/记录值） */
const searchQuery = ref('')
/** 搜索防抖定时器（输入停顿 300ms 才发起请求） */
let searchTimer: ReturnType<typeof setTimeout> | null = null

/** 输入防抖后搜索：重置到第一页并携带关键词重新拉取 */
watch(searchQuery, () => {
  if (searchTimer) clearTimeout(searchTimer)
  searchTimer = setTimeout(() => {
    loadRecords(1)
  }, 300)
})

onUnmounted(() => {
  if (searchTimer) clearTimeout(searchTimer)
})

/** 表单模式：null=关闭，add=新增，CloudRecord=编辑 */
const formOpen = ref<null | 'add' | CloudRecord>(null)
const formRr = ref('')
const formType = ref('A')
const formValue = ref('')
const formTtl = ref<number>(600)
const saving = ref(false)

/** 待删除确认的记录 ID（两段式） */
const confirmDeleteId = ref<string | null>(null)

/** 总页数 */
const totalPages = computed(() => Math.max(1, Math.ceil(total.value / pageSize)))

/** 拉取记录列表（分页；携带搜索关键词走服务端过滤） */
async function loadRecords(p = page.value) {
  busy.value = true
  try {
    const r = await ipc.dnsRecords(
      props.platform,
      props.domain.domainName,
      p,
      pageSize,
      searchQuery.value.trim()
    )
    records.value = r.list
    total.value = r.total
    page.value = p
  } catch (e) {
    ui.toast('加载记录失败：' + (e instanceof Error ? e.message : String(e)))
  } finally {
    busy.value = false
  }
}

/** 打开新增表单（清空并聚焦） */
function openAdd() {
  formOpen.value = 'add'
  formRr.value = ''
  formType.value = 'A'
  formValue.value = ''
  formTtl.value = 600
}

/** 打开编辑表单（回填记录） */
function openEdit(r: CloudRecord) {
  formOpen.value = r
  formRr.value = r.rr
  formType.value = r.recordType
  formValue.value = r.value
  formTtl.value = r.ttl
}

/** 保存表单（新增或更新；失败 toast 显示平台错误） */
async function saveForm() {
  if (!formRr.value.trim()) {
    ui.toast('主机记录不能为空（根域名填 @）')
    return
  }
  if (!formValue.value.trim()) {
    ui.toast('记录值不能为空')
    return
  }
  saving.value = true
  try {
    if (formOpen.value === 'add') {
      await ipc.dnsAddRecord(
        props.platform,
        props.domain.domainName,
        formRr.value.trim(),
        formType.value,
        formValue.value.trim(),
        formTtl.value
      )
      ui.toast('记录已添加')
    } else if (formOpen.value) {
      await ipc.dnsUpdateRecord(
        props.platform,
        props.domain.domainName,
        formOpen.value.recordId,
        formRr.value.trim(),
        formType.value,
        formValue.value.trim(),
        formTtl.value
      )
      ui.toast('记录已更新')
    }
    formOpen.value = null
    await loadRecords()
  } catch (e) {
    ui.toast('保存失败：' + (e instanceof Error ? e.message : String(e)))
  } finally {
    saving.value = false
  }
}

/** 删除记录（两段式确认：首次点击进入确认态，再次点击执行） */
async function deleteRecord(r: CloudRecord) {
  if (confirmDeleteId.value !== r.recordId) {
    confirmDeleteId.value = r.recordId
    // 3 秒未确认自动复原
    setTimeout(() => {
      if (confirmDeleteId.value === r.recordId) confirmDeleteId.value = null
    }, 3000)
    return
  }
  confirmDeleteId.value = null
  try {
    await ipc.dnsDeleteRecord(props.platform, props.domain.domainName, r.recordId)
    ui.toast('记录已删除')
    await loadRecords()
  } catch (e) {
    ui.toast('删除失败：' + (e instanceof Error ? e.message : String(e)))
  }
}

/** 翻页 */
function goto(p: number) {
  if (p < 1 || p > totalPages.value || p === page.value) return
  loadRecords(p)
}

onMounted(loadRecords)
</script>

<template>
  <div class="flex min-h-0 flex-col gap-[10px]">
    <!-- 面包屑 + 搜索 + 操作 -->
    <div class="flex shrink-0 items-center gap-[8px]">
      <button class="btn-ghost shrink-0 px-[10px] py-[6px] text-body-sm" @click="emit('back')">
        ← 返回
      </button>
      <span class="font-mono text-body font-medium text-primary dark:text-primary-dark">
        {{ domain.domainName }}
      </span>
      <span class="whitespace-nowrap text-body-sm text-text-muted dark:text-text-muted-dark">
        {{ total }} 条记录
      </span>
      <input
        v-model="searchQuery"
        class="field-input ml-auto w-[180px] !px-[10px] !py-[7px]"
        placeholder="搜索主机记录 / 记录值…"
        spellcheck="false"
      />
      <button class="btn-primary shrink-0 px-[12px] py-[6px] text-body-sm" @click="openAdd">
        + 添加记录
      </button>
    </div>

    <!-- 行内表单（新增/编辑共用；控件统一 field-input 压缩高度，与 HTTP 工具一致） -->
    <div
      v-if="formOpen"
      class="flex shrink-0 flex-wrap items-end gap-[10px] rounded-md border border-tertiary/40 bg-tertiary-soft/30 p-[10px] dark:border-tertiary-dark/40 dark:bg-tertiary-soft-dark/30"
    >
      <label class="flex flex-col gap-[4px]">
        <span class="field-label text-body-sm">主机记录</span>
        <input
          v-model="formRr"
          class="field-input w-[130px] !px-[10px] !py-[7px] font-mono"
          placeholder="@ / www"
          spellcheck="false"
        />
      </label>
      <label class="flex flex-col gap-[4px]">
        <span class="field-label text-body-sm">类型</span>
        <Select
          :model-value="formType"
          class="!w-[100px]"
          :options="CLOUD_RECORD_TYPES.map((t) => ({ value: t, label: t }))"
          @update:model-value="formType = $event"
        />
      </label>
      <label class="flex min-w-[180px] flex-1 flex-col gap-[4px]">
        <span class="field-label text-body-sm">记录值</span>
        <input
          v-model="formValue"
          class="field-input !px-[10px] !py-[7px] font-mono placeholder:font-sans"
          placeholder="目标 IP / 域名"
          spellcheck="false"
        />
      </label>
      <label class="flex flex-col gap-[4px]">
        <span class="field-label text-body-sm">TTL</span>
        <Select
          :model-value="String(formTtl)"
          class="!w-[100px]"
          :options="TTL_PRESETS.map((t) => ({ value: String(t), label: `${t}s` }))"
          @update:model-value="formTtl = Number($event)"
        />
      </label>
      <div class="flex gap-[8px]">
        <button
          class="btn-primary px-[12px] py-[7px] text-body-sm"
          :disabled="saving"
          @click="saveForm"
        >
          {{ saving ? '保存中…' : '保存' }}
        </button>
        <button class="btn-ghost px-[12px] py-[7px] text-body-sm" @click="formOpen = null">
          取消
        </button>
      </div>
    </div>

    <!-- 记录表格（主机记录列只显示 rr，域名在标题栏；线路/操作列不换行） -->
    <div
      class="min-h-0 flex-1 overflow-auto rounded-lg border border-border bg-surface pr-[2px] dark:border-border-dark dark:bg-surface-dark"
    >
      <p
        v-if="!busy && records.length === 0"
        class="p-[14px] text-body-sm text-text-muted dark:text-text-muted-dark"
      >
        暂无解析记录，点击「添加记录」创建。
      </p>
      <table v-else class="w-full min-w-[520px] border-collapse">
        <thead class="sticky top-0 bg-surface dark:bg-surface-dark">
          <tr
            class="border-b border-border text-left text-body-sm text-text-muted dark:border-border-dark dark:text-text-muted-dark"
          >
            <th class="py-[8px] pl-[12px] pr-[12px] font-medium">主机记录</th>
            <th class="py-[8px] pr-[12px] font-medium">类型</th>
            <th class="py-[8px] pr-[12px] font-medium">TTL</th>
            <th class="py-[8px] pr-[12px] font-medium">记录值</th>
            <th class="py-[8px] pr-[12px] font-medium">线路</th>
            <th class="py-[8px] pr-[12px] text-right font-medium">操作</th>
          </tr>
        </thead>
        <tbody class="text-body-sm">
          <tr
            v-for="r in records"
            :key="r.recordId"
            class="border-b border-border/60 last:border-b-0 dark:border-border-dark/60"
          >
            <td
              class="whitespace-nowrap py-[7px] pl-[12px] pr-[12px] font-mono text-secondary dark:text-secondary-dark"
            >
              {{ r.rr === '@' ? '@' : r.rr }}
            </td>
            <td class="py-[7px] pr-[12px] font-mono">
              <span
                class="rounded px-[6px] py-[1px] whitespace-nowrap font-medium"
                :class="recordTypeBadgeClass(r.recordType)"
              >
                {{ r.recordType }}
              </span>
            </td>
            <td
              class="whitespace-nowrap py-[7px] pr-[12px] font-mono text-text-muted dark:text-text-muted-dark"
            >
              {{ r.ttl }}
            </td>
            <td
              class="break-all py-[7px] pr-[12px] font-mono text-secondary dark:text-secondary-dark"
            >
              {{ r.value }}
            </td>
            <td
              class="whitespace-nowrap py-[7px] pr-[12px] text-text-muted dark:text-text-muted-dark"
            >
              {{ r.line }}
            </td>
            <td class="whitespace-nowrap py-[7px] pr-[12px] text-right">
              <button
                class="mr-[6px] rounded px-[8px] py-[3px] text-body-sm text-info-strong transition-colors hover:bg-info-soft dark:text-info-dark dark:hover:bg-info-soft-dark"
                @click="openEdit(r)"
              >
                编辑
              </button>
              <button
                class="rounded px-[8px] py-[3px] text-body-sm transition-colors"
                :class="
                  confirmDeleteId === r.recordId
                    ? 'bg-danger-soft text-danger-strong dark:bg-danger-soft-dark dark:text-danger-dark'
                    : 'text-danger-strong hover:bg-danger-soft dark:text-danger-dark dark:hover:bg-danger-soft-dark'
                "
                @click="deleteRecord(r)"
              >
                {{ confirmDeleteId === r.recordId ? '确认删除？' : '删除' }}
              </button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- 分页 -->
    <div
      v-if="totalPages > 1"
      class="flex shrink-0 items-center justify-end gap-[8px] text-body-sm"
    >
      <button class="btn-ghost px-[10px] py-[5px]" :disabled="page <= 1" @click="goto(page - 1)">
        上一页
      </button>
      <span class="text-text-muted dark:text-text-muted-dark">{{ page }} / {{ totalPages }}</span>
      <button
        class="btn-ghost px-[10px] py-[5px]"
        :disabled="page >= totalPages"
        @click="goto(page + 1)"
      >
        下一页
      </button>
    </div>
  </div>
</template>
