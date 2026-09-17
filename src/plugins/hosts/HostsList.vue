<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
/**
 * HostsList · hosts 列表模式：结构化条目（启用开关 / IP / 主机名 / 注释 / 增删）
 * 编辑后通过 emit("change", text) 同步回 hosts 文本（保留注释行/空行原位）。
 */
import { computed, ref } from 'vue'
import type { HostsEntry } from './useHosts'
import { parseEntries, validateEntry } from './useHosts'
import { UiButton, UiCheckbox, UiIcon, UiIconButton, UiInput } from '@/core/ui'

const props = defineProps<{ content: string }>()
const emit = defineEmits<{ (e: 'change', text: string): void }>()

const entries = ref<HostsEntry[]>(parseEntries(props.content))

/** 可编辑条目（映射行 + 被注释的映射行（未勾选）；纯注释/空行保留但不在列表显示） */
const editable = computed(() =>
  entries.value.filter((e) => e.raw === undefined || (e.raw.startsWith('#') && e.ip))
)

/** 纯注释/空行数量（提示保留） */
const rawCount = computed(() => entries.value.filter((e) => e.raw !== undefined).length)

const errors = computed(() => editable.value.filter((e) => !e.valid).length)

function sync() {
  emit('change', entriesToText())
}

function entriesToText(): string {
  return entries.value
    .map((e) => {
      if (e.raw !== undefined) return e.raw
      const hostStr = e.hosts.join(' ')
      let line = `${e.ip} ${hostStr}`.trimEnd()
      if (!e.enabled) line = `# ${line}`
      if (e.comment.trim()) line += ` ${e.comment.trim()}`
      return line
    })
    .join('\n')
}

function updateEntry(e: HostsEntry) {
  const check = validateEntry(e.ip, e.hosts)
  e.valid = check.valid
  e.error = check.error
  sync()
}

function onIpInput(e: HostsEntry, v: string) {
  e.ip = v.trim()
  updateEntry(e)
}

function onHostsInput(e: HostsEntry, v: string) {
  e.hosts = v
    .split(/[,\s]+/)
    .map((s) => s.trim())
    .filter(Boolean)
  updateEntry(e)
}

function onCommentInput(e: HostsEntry, v: string) {
  e.comment = v.trim() ? `# ${v.trim()}` : ''
  updateEntry(e)
}

function toggleEnabled(e: HostsEntry) {
  e.enabled = !e.enabled
  sync()
}

function remove(e: HostsEntry) {
  entries.value = entries.value.filter((x) => x.id !== e.id)
  sync()
}

function addRow() {
  const id = `n${Date.now()}`
  entries.value.push({
    id,
    enabled: true,
    ip: '',
    hosts: [],
    comment: '',
    valid: false,
    error: 'IP 无效',
  })
  sync()
}
</script>

<template>
  <div class="flex h-full flex-col gap-[8px]">
    <!-- 统计 + 操作（固定） -->
    <div class="flex shrink-0 items-center justify-between">
      <div class="flex items-center gap-[14px]">
        <span class="text-body-sm font-medium text-secondary dark:text-secondary-dark">
          {{ editable.length }} 条映射
        </span>
        <span
          v-if="errors > 0"
          class="text-body-sm font-medium text-tertiary-strong dark:text-tertiary-dark"
        >
          ⚠ {{ errors }} 处无效（保存将被阻止）
        </span>
        <span v-else class="text-body-sm text-text-muted dark:text-text-muted-dark">
          语法检查通过
        </span>
        <span v-if="rawCount > 0" class="text-body-sm text-text-muted dark:text-text-muted-dark">
          {{ rawCount }} 行注释/空行将原样保留
        </span>
      </div>
      <UiButton variant="ghost" @click="addRow">+ 新增映射</UiButton>
    </div>

    <!-- 表头（固定） -->
    <div
      class="grid shrink-0 grid-cols-[40px_170px_1fr_200px_44px] items-center gap-[8px] px-[12px] text-body-sm text-text-muted dark:text-text-muted-dark"
    >
      <span>启用</span>
      <span>IP 地址</span>
      <span>主机名（多个用空格/逗号分隔）</span>
      <span>注释</span>
      <span />
    </div>

    <!-- 条目区（仅条目滚动） -->
    <UiScrollArea as-child axis="vertical">
      <div class="min-h-0 flex-1">
        <div
          v-for="e in editable"
          :key="e.id"
          class="mb-[8px] grid grid-cols-[40px_170px_1fr_200px_44px] items-center gap-[8px] rounded-md border px-[12px] py-[8px]"
          :class="
            e.valid
              ? 'border-border bg-surface dark:border-border-dark dark:bg-surface-dark'
              : 'border-tertiary/40 bg-tertiary-soft/30 dark:border-tertiary-dark/40 dark:bg-tertiary-soft-dark/30'
          "
        >
          <UiCheckbox
            :model-value="e.enabled"
            :title="e.enabled ? '点击禁用（行首加 #）' : '点击启用'"
            @update:model-value="toggleEnabled(e)"
          />
          <UiInput
            :model-value="e.ip"
            class="font-mono"
            placeholder="127.0.0.1"
            spellcheck="false"
            @update:model-value="onIpInput(e, String($event))"
          />
          <UiInput
            :model-value="e.hosts.join(' ')"
            class="font-mono"
            placeholder="example.com www.example.com"
            spellcheck="false"
            @update:model-value="onHostsInput(e, String($event))"
          />
          <UiInput
            :model-value="e.comment.replace(/^#\s*/, '')"
            placeholder="备注（可选）"
            @update:model-value="onCommentInput(e, String($event))"
          />
          <UiIconButton label="删除此条" size="sm" @click="remove(e)">
            <UiIcon name="trash" :size="14" />
          </UiIconButton>
        </div>

        <p
          v-if="!editable.length"
          class="py-[24px] text-center text-body-sm text-text-muted dark:text-text-muted-dark"
        >
          暂无映射条目，点击「+ 新增映射」添加
        </p>
      </div>
    </UiScrollArea>
  </div>
</template>
