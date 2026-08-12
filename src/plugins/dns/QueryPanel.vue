<script setup lang="ts">
/**
 * DNS 查询 · 多类型 + 多服务器对比（dig 风格）
 * 预设服务器勾选 + 自定义服务器追加；每台服务器独立结果卡片。
 */
import { computed, ref } from 'vue'
import { ipc } from './ipc'
import { useUiStore } from '@/stores/ui'
import type { ServerQueryResult } from './contracts'
import { UiButton, UiInput, UiSelect as Select, UiTableCell } from '@/core/ui'
import {
  DNS_SERVERS,
  RECORD_TYPES,
  isValidDomain,
  isValidServer,
  recordTypeBadgeClass,
  serverLabel,
} from './useDns'

const ui = useUiStore()

const domain = ref('')
const rtype = ref<string>('A')
const selected = ref<string[]>(['system', '223.5.5.5', '119.29.29.29'])
const customServer = ref('')
const customServers = ref<string[]>([])
const busy = ref(false)
const results = ref<ServerQueryResult[]>([])

/** 有效服务器列表（预设勾选 + 自定义） */
const servers = computed(() => [
  ...DNS_SERVERS.filter((s) => selected.value.includes(s.addr)).map((s) => s.addr),
  ...customServers.value,
])

/** 是否全部成功（顶部状态提示用） */
const allOk = computed(() => results.value.length > 0 && results.value.every((r) => r.ok))

/** 添加自定义服务器（校验格式；重复忽略） */
function addCustom() {
  const v = customServer.value.trim()
  if (!isValidServer(v)) {
    ui.toast('服务器地址无效（IPv4/IPv6/域名，如 8.8.8.8）')
    return
  }
  if (customServers.value.includes(v) || servers.value.includes(v)) {
    ui.toast('该服务器已在列表中')
    return
  }
  customServers.value.push(v)
  customServer.value = ''
}

/** 移除自定义服务器 */
function removeCustom(addr: string) {
  customServers.value = customServers.value.filter((s) => s !== addr)
}

/** 执行查询（Enter 与按钮均触发） */
async function run() {
  if (!isValidDomain(domain.value)) {
    ui.toast('请输入合法域名（如 example.com）')
    return
  }
  if (servers.value.length === 0) {
    ui.toast('请至少选择一台 DNS 服务器')
    return
  }
  busy.value = true
  try {
    results.value = await ipc.dnsQuery(domain.value.trim(), rtype.value, servers.value)
  } catch (e) {
    ui.toast('查询失败：' + (e instanceof Error ? e.message : String(e)))
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-[12px]">
    <!-- 查询条件区（固定，不随结果滚动） -->
    <div
      class="flex shrink-0 flex-col gap-[10px] border-b border-border pb-[12px] dark:border-border-dark"
    >
      <div class="flex flex-wrap items-center gap-[8px]">
        <UiInput
          v-model="domain"
          class="min-w-[240px] flex-1 font-mono placeholder:font-sans"
          placeholder="域名，如 example.com"
          spellcheck="false"
          @keyup.enter="run"
        />
        <Select
          v-model="rtype"
          class="!w-[110px] shrink-0"
          title="记录类型"
          :options="RECORD_TYPES.map((t) => ({ value: t, label: t }))"
        />
        <UiButton variant="primary" class="shrink-0" :loading="busy" @click="run">
          {{ busy ? '查询中…' : '查询' }}
        </UiButton>
      </div>

      <!-- 服务器选择：预设 chips + 自定义追加 -->
      <div class="flex flex-wrap items-center gap-[8px]">
        <span class="text-body-sm text-text-muted dark:text-text-muted-dark">服务器</span>
        <button
          v-for="s in DNS_SERVERS"
          :key="s.addr"
          class="rounded-md px-[10px] py-[5px] text-body-sm font-medium transition-colors"
          :class="
            selected.includes(s.addr)
              ? 'bg-tertiary-soft text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark'
              : 'bg-neutral text-secondary hover:text-primary dark:bg-neutral-dark dark:text-secondary-dark dark:hover:text-primary-dark'
          "
          @click="
            selected.includes(s.addr)
              ? (selected = selected.filter((a) => a !== s.addr))
              : selected.push(s.addr)
          "
        >
          {{ s.label }}
        </button>

        <!-- 自定义服务器 chip（可移除） -->
        <span
          v-for="c in customServers"
          :key="c"
          class="flex items-center gap-[6px] rounded-md bg-purple-soft px-[10px] py-[5px] font-mono text-body-sm text-purple-strong dark:bg-purple-soft-dark dark:text-purple-dark"
        >
          {{ c }}
          <button class="text-purple-strong/70 hover:text-purple-strong" @click="removeCustom(c)">
            ✕
          </button>
        </span>

        <UiInput
          v-model="customServer"
          class="w-[160px] font-mono placeholder:font-sans"
          placeholder="自定义 IP:端口"
          spellcheck="false"
          @keyup.enter="addCustom"
        />
        <UiButton variant="ghost" size="sm" class="shrink-0" @click="addCustom"> 添加 </UiButton>
      </div>
    </div>

    <!-- 结果区：每台服务器一张卡片 -->
    <div class="min-h-0 flex-1 space-y-[10px] overflow-y-auto pr-[2px]">
      <p v-if="results.length === 0" class="text-body-sm text-text-muted dark:text-text-muted-dark">
        输入域名并选择记录类型与服务器，点击「查询」开始（支持 PTR：直接填 IP）。
      </p>

      <div
        v-for="r in results"
        :key="r.server"
        class="rounded-lg border border-border bg-surface p-[12px] dark:border-border-dark dark:bg-surface-dark"
      >
        <!-- 卡片头：服务器 + 耗时 + 状态 -->
        <div class="mb-[8px] flex items-center gap-[8px]">
          <span class="font-mono text-body font-medium text-primary dark:text-primary-dark">
            {{ serverLabel(r.server) }}
          </span>
          <span
            class="rounded-full px-[8px] py-[2px] text-body-sm font-medium"
            :class="
              r.ok
                ? 'bg-success-soft text-success-strong dark:bg-success-soft-dark dark:text-success-dark'
                : 'bg-danger-soft text-danger-strong dark:bg-danger-soft-dark dark:text-danger-dark'
            "
          >
            {{ r.ok ? '成功' : '失败' }}
          </span>
          <span v-if="r.ok" class="text-body-sm text-text-muted dark:text-text-muted-dark">
            {{ r.elapsedMs }} ms · {{ r.records.length }} 条
          </span>
        </div>

        <!-- 失败：错误原因 -->
        <p v-if="!r.ok" class="font-mono text-body-sm text-danger-strong dark:text-danger-dark">
          {{ r.error }}
        </p>

        <!-- 成功：记录表格 -->
        <table v-else-if="r.records.length > 0" class="w-full border-collapse">
          <thead>
            <tr
              class="border-b border-border text-left text-body-sm text-text-muted dark:border-border-dark dark:text-text-muted-dark"
            >
              <UiTableCell as="th" class="py-[4px] pr-[12px]">类型</UiTableCell>
              <UiTableCell as="th" class="py-[4px] pr-[12px]">名称</UiTableCell>
              <UiTableCell as="th" class="py-[4px] pr-[12px]">TTL</UiTableCell>
              <UiTableCell as="th" class="py-[4px]">值</UiTableCell>
            </tr>
          </thead>
          <tbody class="text-body-sm">
            <tr
              v-for="(rec, i) in r.records"
              :key="i"
              class="border-b border-border/60 last:border-b-0 dark:border-border-dark/60"
            >
              <UiTableCell content="technical" class="py-[5px] pr-[12px]">
                <span
                  class="rounded px-[6px] py-[1px] text-body-sm font-medium"
                  :class="recordTypeBadgeClass(rec.recordType)"
                >
                  {{ rec.recordType }}
                </span>
              </UiTableCell>
              <UiTableCell
                content="technical"
                class="py-[5px] pr-[12px] text-secondary dark:text-secondary-dark"
              >
                {{ rec.name }}
              </UiTableCell>
              <UiTableCell
                content="numeric"
                class="py-[5px] pr-[12px] text-text-muted dark:text-text-muted-dark"
              >
                {{ rec.ttl }}
              </UiTableCell>
              <UiTableCell
                content="code"
                class="break-all py-[5px] text-secondary dark:text-secondary-dark"
              >
                {{ rec.value }}
              </UiTableCell>
            </tr>
          </tbody>
        </table>
        <p v-else class="text-body-sm text-text-muted dark:text-text-muted-dark">
          查询成功，无应答记录。
        </p>
      </div>

      <p
        v-if="allOk && results.length > 1"
        class="text-body-sm text-success-strong dark:text-success-dark"
      >
        ✓ 全部服务器查询成功
      </p>
    </div>
  </div>
</template>
