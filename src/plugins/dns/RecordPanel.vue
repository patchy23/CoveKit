<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { ipc } from './ipc'
import { useUiStore } from '@/stores/ui'
import type { CloudDomain, CloudRecord, DnsPlatform } from './contracts'
import { recordTypeBadgeClass } from './useDns'
import DnsRecordForm from './DnsRecordForm.vue'
import { UiButton, UiPagination, UiSearchInput, UiTable, UiTableCell } from '@/core/ui'

const props = defineProps<{
  platform: DnsPlatform
  domain: CloudDomain
}>()

const emit = defineEmits<{ back: [] }>()

const ui = useUiStore()

const records = ref<CloudRecord[]>([])
const total = ref(0)
const page = ref(1)
const pageSize = 50
const busy = ref(false)
const searchQuery = ref('')
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
    <div class="flex shrink-0 items-center gap-[8px]">
      <UiButton variant="ghost" size="sm" class="shrink-0" @click="emit('back')"> ← 返回 </UiButton>
      <span class="font-mono text-body font-medium text-primary dark:text-primary-dark">
        {{ domain.domainName }}
      </span>
      <span class="whitespace-nowrap text-body-sm text-text-muted dark:text-text-muted-dark">
        {{ total }} 条记录
      </span>
      <UiSearchInput
        v-model="searchQuery"
        size="sm"
        class="ml-auto w-[180px]"
        placeholder="搜索主机记录 / 记录值…"
      />
      <UiButton variant="primary" size="sm" class="shrink-0" @click="openAdd">
        + 添加记录
      </UiButton>
    </div>

    <DnsRecordForm
      v-if="formOpen"
      :rr="formRr"
      :record-type="formType"
      :value="formValue"
      :ttl="formTtl"
      :saving="saving"
      @cancel="formOpen = null"
      @save="saveForm"
      @update:rr="formRr = $event"
      @update:record-type="formType = $event"
      @update:value="formValue = $event"
      @update:ttl="formTtl = $event"
    />

    <UiScrollArea as-child axis="both">
      <div
        class="min-h-0 flex-1 rounded-lg border border-border bg-surface pr-[2px] dark:border-border-dark dark:bg-surface-dark"
      >
        <p
          v-if="!busy && records.length === 0"
          class="p-[14px] text-body-sm text-text-muted dark:text-text-muted-dark"
        >
          暂无解析记录，点击「添加记录」创建。
        </p>
        <UiTable v-else :framed="false" :styled="false" table-class="min-w-[520px]">
          <thead class="sticky top-0 bg-surface dark:bg-surface-dark">
            <tr
              class="border-b border-border text-left text-body-sm text-text-muted dark:border-border-dark dark:text-text-muted-dark"
            >
              <UiTableCell as="th" class="py-[8px] pl-[12px] pr-[12px]">主机记录</UiTableCell>
              <UiTableCell as="th" class="py-[8px] pr-[12px]">类型</UiTableCell>
              <UiTableCell as="th" class="py-[8px] pr-[12px]">TTL</UiTableCell>
              <UiTableCell as="th" class="py-[8px] pr-[12px]">记录值</UiTableCell>
              <UiTableCell as="th" class="py-[8px] pr-[12px]">线路</UiTableCell>
              <UiTableCell as="th" align="right" class="py-[8px] pr-[12px]">操作</UiTableCell>
            </tr>
          </thead>
          <tbody class="text-body-sm">
            <tr
              v-for="r in records"
              :key="r.recordId"
              class="border-b border-border/60 last:border-b-0 dark:border-border-dark/60"
            >
              <UiTableCell
                content="technical"
                class="whitespace-nowrap py-[7px] pl-[12px] pr-[12px] text-secondary dark:text-secondary-dark"
              >
                {{ r.rr === '@' ? '@' : r.rr }}
              </UiTableCell>
              <UiTableCell content="technical" class="py-[7px] pr-[12px]">
                <span
                  class="rounded px-[6px] py-[1px] whitespace-nowrap font-medium"
                  :class="recordTypeBadgeClass(r.recordType)"
                >
                  {{ r.recordType }}
                </span>
              </UiTableCell>
              <UiTableCell
                content="numeric"
                class="whitespace-nowrap py-[7px] pr-[12px] text-text-muted dark:text-text-muted-dark"
              >
                {{ r.ttl }}
              </UiTableCell>
              <UiTableCell
                content="code"
                class="break-all py-[7px] pr-[12px] text-secondary dark:text-secondary-dark"
              >
                {{ r.value }}
              </UiTableCell>
              <UiTableCell
                content="text"
                class="whitespace-nowrap py-[7px] pr-[12px] text-text-muted dark:text-text-muted-dark"
              >
                {{ r.line }}
              </UiTableCell>
              <UiTableCell
                content="action"
                align="right"
                class="whitespace-nowrap py-[7px] pr-[12px]"
              >
                <UiButton
                  variant="ghost"
                  size="xs"
                  class="mr-[2px] text-info-strong dark:text-info-dark"
                  @click="openEdit(r)"
                >
                  编辑
                </UiButton>
                <UiButton
                  variant="ghost"
                  size="xs"
                  :class="
                    confirmDeleteId === r.recordId
                      ? 'bg-danger-soft text-danger-strong dark:bg-danger-soft-dark dark:text-danger-dark'
                      : 'text-danger-strong hover:bg-danger-soft dark:text-danger-dark dark:hover:bg-danger-soft-dark'
                  "
                  @click="deleteRecord(r)"
                >
                  {{ confirmDeleteId === r.recordId ? '确认删除？' : '删除' }}
                </UiButton>
              </UiTableCell>
            </tr>
          </tbody>
        </UiTable>
      </div>
    </UiScrollArea>

    <UiPagination
      v-if="totalPages > 1"
      :model-value="page"
      :total-pages="totalPages"
      size="sm"
      @update:model-value="goto"
    />
  </div>
</template>
