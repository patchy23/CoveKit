<script setup lang="ts">
/**
 * EditorDialog · 远程文件编辑弹窗
 * 文件管理页签双击文件打开；CodeMirror 6 可编辑模式（语法高亮/行号/缩进线，
 * 与 CodeViewer 同款配色）；保存后由父组件回写服务器（后端 IPC 接入前为 mock）。
 */
import { onMounted, onUnmounted, ref } from "vue";
import { EditorView, lineNumbers } from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";
import { json } from "@codemirror/lang-json";
import { xml } from "@codemirror/lang-xml";

const props = defineProps<{
  /** 远程文件完整路径 */
  path: string;
  /** 文件内容 */
  content: string;
}>();

const emit = defineEmits<{
  (e: "save", content: string): void;
  (e: "cancel"): void;
}>();

const host = ref<HTMLElement | null>(null);
const dirty = ref(false);
let view: EditorView | null = null;

/* ── 语法高亮（与 CodeViewer 同款 GitHub 配色，CSS 变量随主题） ── */
const highlight = HighlightStyle.define([
  { tag: t.keyword, color: "var(--cm-keyword)" },
  { tag: [t.propertyName, t.attributeName], color: "var(--cm-property)" },
  { tag: [t.string, t.special(t.string)], color: "var(--cm-string)" },
  { tag: [t.number, t.bool, t.null], color: "var(--cm-number)" },
  { tag: [t.tagName, t.typeName], color: "var(--cm-tag)" },
  { tag: [t.angleBracket, t.paren, t.brace, t.bracket, t.separator], color: "var(--cm-punct)" },
  { tag: t.comment, color: "var(--cm-comment)", fontStyle: "italic" },
  { tag: t.operator, color: "var(--cm-punct)" },
]);

/** 按扩展名选择语法（未匹配返回空扩展 = 纯文本） */
function langFor(path: string) {
  const ext = path.split(".").pop()?.toLowerCase();
  if (ext === "json") return json();
  if (ext === "xml" || ext === "html" || ext === "htm" || ext === "svg") return xml();
  return [];
}

function createEditor() {
  view = new EditorView({
    parent: host.value!,
    state: EditorState.create({
      doc: props.content,
      extensions: [
        lineNumbers(),
        syntaxHighlighting(highlight),
        langFor(props.path),
        // 编辑监听：内容变化标记 dirty
        EditorView.updateListener.of((u) => {
          if (u.docChanged) dirty.value = true;
        }),
        EditorView.theme({
          "&": { height: "100%", fontSize: "13px" },
          ".cm-scroller": {
            fontFamily: "var(--font-mono)",
            lineHeight: "1.5",
            overflow: "auto",
          },
          ".cm-content": { padding: "10px 0" },
          ".cm-line": { padding: "0 12px" },
          ".cm-cursor": { borderLeftColor: "var(--color-tertiary)" },
          ".cm-gutters": {
            background: "transparent",
            borderRight: "1px solid var(--color-border)",
            color: "var(--color-text-muted)",
            fontSize: "12px",
          },
        }),
      ],
    }),
  });
}

function save() {
  emit("save", view?.state.doc.toString() ?? props.content);
}

function cancel() {
  emit("cancel");
}

onMounted(createEditor);
onUnmounted(() => view?.destroy());
</script>

<template>
  <Teleport to="body">
    <div
      class="fixed inset-0 z-[150] grid place-items-center bg-black/30 p-[40px]"
      @click.self="cancel"
    >
      <div
        class="flex h-full w-full max-w-[820px] flex-col overflow-hidden rounded-lg border border-border bg-surface shadow-[0_16px_48px_rgba(16,24,40,0.25)] dark:border-border-dark dark:bg-surface-dark"
      >
        <!-- 标题栏：路径 + dirty 标记 -->
        <div
          class="flex shrink-0 items-center gap-[10px] border-b border-border px-[16px] py-[10px] dark:border-border-dark"
        >
          <span class="font-mono text-body font-medium text-primary dark:text-primary-dark">
            {{ path }}
          </span>
          <span
            v-if="dirty"
            class="rounded-[4px] bg-warning-soft px-[6px] py-[1px] text-caption font-medium text-warning-strong dark:bg-warning-soft-dark dark:text-warning-dark"
          >
            已修改
          </span>
          <div class="ml-auto flex items-center gap-[8px]">
            <button class="btn-ghost !px-[10px] !py-[4px] text-body-sm" @click="cancel">
              取消
            </button>
            <button class="btn-primary !h-[32px] !px-[14px] text-body-sm" @click="save">
              保存
            </button>
          </div>
        </div>

        <!-- CodeMirror 编辑区 -->
        <div ref="host" class="min-h-0 flex-1 overflow-hidden bg-surface-muted dark:bg-surface-muted-dark" />
      </div>
    </div>
  </Teleport>
</template>
