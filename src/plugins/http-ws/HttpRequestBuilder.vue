<script setup lang="ts">
/**
 * HttpRequestBuilder · 请求构建区（Postman 式 Params / Headers / Body 分页签）
 * Params/Headers 键值表走 UiKvEditor（原两处复制实现已收编为公共组件）。
 */
import { ref } from 'vue'
import type { KvRow } from './useHttp'
import { UiCodeEditor, UiKvEditor, UiSelect as Select, UiTabs } from '@/core/ui'

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
    <UiKvEditor
      v-if="!props.headersOnly && tab === 'params'"
      :rows="props.params"
      key-label="参数名"
      add-label="添加参数"
      remove-label="删除参数"
      @update:rows="emit('update:params', $event)"
    />

    <!-- Headers：键值表格（HTTP 与 WS 共用） -->
    <UiKvEditor
      v-if="tab === 'headers' || props.headersOnly"
      :rows="props.headers"
      key-label="Header 名"
      key-placeholder="Accept"
      value-placeholder="application/json"
      add-label="添加 Header"
      remove-label="删除 Header"
      @update:rows="emit('update:headers', $event)"
    />

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
