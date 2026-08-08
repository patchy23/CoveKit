<script setup lang="ts">
/**
 * HttpRequestBuilder · 请求构建区（Postman 式 Params / Headers / Body 分页签）
 */
import { ref } from "vue";
import type { KvRow } from "./useHttp";
import { newKvId } from "./useHttp";
import LineNumberTextarea from "@/core/ui/LineNumberTextarea.vue";
import Select from "@/features/ui/Select.vue";

const props = defineProps<{
  params: KvRow[];
  headers: KvRow[];
  bodyMode: "none" | "json" | "text";
  body: string;
  /** 仅显示 Headers 表格（WebSocket 模式：无 Params/Body） */
  headersOnly?: boolean;
}>();

const emit = defineEmits<{
  (e: "update:params", v: KvRow[]): void;
  (e: "update:headers", v: KvRow[]): void;
  (e: "update:bodyMode", v: "none" | "json" | "text"): void;
  (e: "update:body", v: string): void;
}>();

const tab = ref<"params" | "headers" | "body">("params");

const JSON_PLACEHOLDER = '{\n  "key": "value"\n}';

function addRow(rows: KvRow[], kind: "params" | "headers") {
  const next = [...rows, { id: newKvId(), key: "", value: "" }];
  if (kind === "params") emit("update:params", next);
  else emit("update:headers", next);
}

function removeRow(rows: KvRow[], id: string, kind: "params" | "headers") {
  const next = rows.filter((r) => r.id !== id);
  if (kind === "params") emit("update:params", next);
  else emit("update:headers", next);
}

function setRow(
  rows: KvRow[],
  id: string,
  field: "key" | "value",
  v: string,
  kind: "params" | "headers"
) {
  const next = rows.map((r) => (r.id === id ? { ...r, [field]: v } : r));
  if (kind === "params") emit("update:params", next);
  else emit("update:headers", next);
}

const tabClass = (active: boolean) =>
  active
    ? "border-b-[2px] border-tertiary-strong pb-[6px] font-medium text-tertiary-strong dark:border-tertiary-dark dark:text-tertiary-dark"
    : "border-b-[2px] border-transparent pb-[6px] text-secondary hover:text-primary dark:text-secondary-dark dark:hover:text-primary-dark";
</script>

<template>
  <div class="flex flex-col gap-[10px]">
    <!-- 分页签（WS 模式隐藏，仅显示 Headers） -->
    <div v-if="!props.headersOnly" class="flex shrink-0 gap-[16px] text-body">
      <button class="transition-colors" :class="tabClass(tab === 'params')" @click="tab = 'params'">
        Params
      </button>
      <button
        class="transition-colors"
        :class="tabClass(tab === 'headers')"
        @click="tab = 'headers'"
      >
        Headers
      </button>
      <button class="transition-colors" :class="tabClass(tab === 'body')" @click="tab = 'body'">
        Body
      </button>
    </div>

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
        <input
          :value="r.key"
          class="field-input !px-[10px] !py-[7px] font-mono"
          placeholder="key"
          spellcheck="false"
          @input="
            setRow(props.params, r.id, 'key', ($event.target as HTMLInputElement).value, 'params')
          "
        />
        <input
          :value="r.value"
          class="field-input !px-[10px] !py-[7px] font-mono"
          placeholder="value"
          spellcheck="false"
          @input="
            setRow(props.params, r.id, 'value', ($event.target as HTMLInputElement).value, 'params')
          "
        />
        <button
          class="grid h-[34px] place-items-center rounded-md text-text-muted transition-colors hover:bg-tertiary-soft hover:text-tertiary-strong dark:text-text-muted-dark dark:hover:bg-tertiary-soft-dark dark:hover:text-tertiary-dark"
          title="删除"
          @click="removeRow(props.params, r.id, 'params')"
        >
          <svg
            width="13"
            height="13"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
          >
            <path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3" />
          </svg>
        </button>
      </div>
      <button class="btn-ghost text-body-sm" @click="addRow(props.params, 'params')">
        + 添加参数
      </button>
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
        <input
          :value="r.key"
          class="field-input !px-[10px] !py-[7px] font-mono"
          placeholder="Accept"
          spellcheck="false"
          @input="
            setRow(props.headers, r.id, 'key', ($event.target as HTMLInputElement).value, 'headers')
          "
        />
        <input
          :value="r.value"
          class="field-input !px-[10px] !py-[7px] font-mono"
          placeholder="application/json"
          spellcheck="false"
          @input="
            setRow(
              props.headers,
              r.id,
              'value',
              ($event.target as HTMLInputElement).value,
              'headers'
            )
          "
        />
        <button
          class="grid h-[34px] place-items-center rounded-md text-text-muted transition-colors hover:bg-tertiary-soft hover:text-tertiary-strong dark:text-text-muted-dark dark:hover:bg-tertiary-soft-dark dark:hover:text-tertiary-dark"
          title="删除"
          @click="removeRow(props.headers, r.id, 'headers')"
        >
          <svg
            width="13"
            height="13"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
          >
            <path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3" />
          </svg>
        </button>
      </div>
      <button class="btn-ghost text-body-sm" @click="addRow(props.headers, 'headers')">
        + 添加 Header
      </button>
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
      <LineNumberTextarea
        v-if="props.bodyMode !== 'none'"
        :model-value="props.body"
        min-height="180px"
        :placeholder="props.bodyMode === 'json' ? JSON_PLACEHOLDER : '请求体内容'"
        @update:model-value="emit('update:body', $event)"
      />
      <p v-else class="text-body-sm text-text-muted dark:text-text-muted-dark">
        GET 等无请求体方法默认 none，切换方法后自动隐藏
      </p>
    </div>
  </div>
</template>
