<script setup lang="ts">
/**
 * HttpRequestBuilder · 请求构建区（Postman 式 Params / Headers / Body 分页签）
 */
import { ref } from 'vue'
import type { KvRow } from './useHttp'
import { newKvId } from './useHttp'
import {
  UiButton,
  UiCodeEditor,
  UiIcon,
  UiIconButton,
  UiInput,
  UiSelect as Select,
  UiTabs,
} from '@/core/ui'

const props = defineProps<{
  params: KvRow[]
  headers: KvRow[]
  bodyMode: 'none' | 'json' | 'text'
  body: string
  /** 仅显示 Headers 表格（WebSocket 模式：无 Params/Body） */
  headersOnly?: boolean
}>()

const emit = defineEmits<{
  (e: 'update:params', v: KvRow[]): void
  (e: 'update:headers', v: KvRow[]): void
  (e: 'update:bodyMode', v: 'none' | 'json' | 'text'): void
  (e: 'update:body', v: string): void
}>()

const tab = ref<'params' | 'headers' | 'body'>('params')

const JSON_PLACEHOLDER = '{\n  "key": "value"\n}'

function addRow(rows: KvRow[], kind: 'params' | 'headers') {
  const next = [...rows, { id: newKvId(), key: '', value: '' }]
  if (kind === 'params') emit('update:params', next)
  else emit('update:headers', next)
}

function removeRow(rows: KvRow[], id: string, kind: 'params' | 'headers') {
  const next = rows.filter((r) => r.id !== id)
  if (kind === 'params') emit('update:params', next)
  else emit('update:headers', next)
}

function setRow(
  rows: KvRow[],
  id: string,
  field: 'key' | 'value',
  v: string,
  kind: 'params' | 'headers'
) {
  const next = rows.map((r) => (r.id === id ? { ...r, [field]: v } : r))
  if (kind === 'params') emit('update:params', next)
  else emit('update:headers', next)
}
</script>

<template>
  <div class="flex flex-col gap-[10px]">
    <!-- 分页签（WS 模式隐藏，仅显示 Headers） -->
    <UiTabs
      v-if="!props.headersOnly"
      v-model="tab"
      variant="line"
      size="sm"
      :items="[
        { value: 'params', label: 'Params' },
        { value: 'headers', label: 'Headers' },
        { value: 'body', label: 'Body' },
      ]"
    />

    <!-- Params：键值表格，自动拼接到 URL query -->
    <div v-if="!props.headersOnly && tab === 'params'">
      <div
        class="mb-[6px] grid grid-cols-[1fr_1fr_36px] gap-[8px] px-[2px] text-caption font-medium text-text-muted dark:text-text-muted-dark"
      >
        <span>参数名</span>
        <span>值</span>
        <span />
      </div>
      <div
        v-for="r in props.params"
        :key="r.id"
        class="mb-[6px] grid grid-cols-[1fr_1fr_36px] gap-[8px]"
      >
        <UiInput
          :value="r.key"
          size="sm"
          class="font-mono"
          placeholder="key"
          spellcheck="false"
          @update:model-value="setRow(props.params, r.id, 'key', String($event), 'params')"
        />
        <UiInput
          :value="r.value"
          size="sm"
          class="font-mono"
          placeholder="value"
          spellcheck="false"
          @update:model-value="setRow(props.params, r.id, 'value', String($event), 'params')"
        />
        <UiIconButton label="删除参数" size="sm" @click="removeRow(props.params, r.id, 'params')">
          <UiIcon name="trash" :size="13" />
        </UiIconButton>
      </div>
      <UiButton variant="ghost" size="sm" @click="addRow(props.params, 'params')">
        + 添加参数
      </UiButton>
    </div>

    <!-- Headers：键值表格（HTTP 与 WS 共用） -->
    <div v-if="tab === 'headers' || props.headersOnly">
      <div
        class="mb-[6px] grid grid-cols-[1fr_1fr_36px] gap-[8px] px-[2px] text-caption font-medium text-text-muted dark:text-text-muted-dark"
      >
        <span>Header 名</span>
        <span>值</span>
        <span />
      </div>
      <div
        v-for="r in props.headers"
        :key="r.id"
        class="mb-[6px] grid grid-cols-[1fr_1fr_36px] gap-[8px]"
      >
        <UiInput
          :value="r.key"
          size="sm"
          class="font-mono"
          placeholder="Accept"
          spellcheck="false"
          @update:model-value="setRow(props.headers, r.id, 'key', String($event), 'headers')"
        />
        <UiInput
          :value="r.value"
          size="sm"
          class="font-mono"
          placeholder="application/json"
          spellcheck="false"
          @update:model-value="setRow(props.headers, r.id, 'value', String($event), 'headers')"
        />
        <UiIconButton
          label="删除 Header"
          size="sm"
          @click="removeRow(props.headers, r.id, 'headers')"
        >
          <UiIcon name="trash" :size="13" />
        </UiIconButton>
      </div>
      <UiButton variant="ghost" size="sm" @click="addRow(props.headers, 'headers')">
        + 添加 Header
      </UiButton>
    </div>

    <!-- Body：模式选择 + 内容 -->
    <div v-if="!props.headersOnly && tab === 'body'" class="flex flex-col gap-[8px]">
      <Select
        :model-value="props.bodyMode"
        class="!w-[180px] shrink-0"
        title="请求体模式"
        :options="[
          { value: 'none', label: 'none（无请求体）' },
          { value: 'json', label: 'raw · JSON' },
          { value: 'text', label: 'raw · 文本' },
        ]"
        @update:model-value="emit('update:bodyMode', $event as 'none' | 'json' | 'text')"
      />
      <UiCodeEditor
        v-if="props.bodyMode !== 'none'"
        :model-value="props.body"
        height="180px"
        :placeholder="props.bodyMode === 'json' ? JSON_PLACEHOLDER : '请求体内容'"
        mode="minimal"
        @update:model-value="emit('update:body', $event)"
      />
      <p v-else class="text-body-sm text-text-muted dark:text-text-muted-dark">
        GET 等无请求体方法默认 none，切换方法后自动隐藏
      </p>
    </div>
  </div>
</template>
