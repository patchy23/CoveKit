<script setup lang="ts">
/**
 * FoldableOutput · 可折叠的语法高亮输出区（JSON/XML 格式化输出用）
 * 按缩进分层：每行记录缩进深度，块起始行（有更深子行）显示折叠箭头，
 * 点击折叠隐藏子树并显示「⋯ N 行」占位（再点展开）；行号随折叠跳号。
 */
import { computed, ref } from "vue";

const props = defineProps<{
  /** 原文（行号与缩进深度依据） */
  text: string;
  /** 语法高亮后的 HTML（按行切分渲染；为空时显示原文纯文本） */
  html: string;
}>();

interface Line {
  depth: number;
  html: string;
}

/** 转义纯文本（高亮缺失行兜底） */
function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

const lines = computed<Line[]>(() => {
  const src = props.text.split("\n");
  const htmlParts = props.html ? props.html.split("\n") : [];
  return src.map((raw, i) => ({
    depth: raw.match(/^\s*/)?.[0].length ?? 0,
    html: htmlParts[i] ?? escapeHtml(raw),
  }));
});

/** 折叠的块起始行号集合（0-based） */
const folded = ref<Set<number>>(new Set());

function hasChildren(i: number): boolean {
  return i + 1 < lines.value.length && lines.value[i + 1].depth > lines.value[i].depth;
}

/** 行是否被某个折叠块覆盖 */
function isHidden(i: number): boolean {
  for (const f of folded.value) {
    if (f < i && lines.value[f].depth < lines.value[i].depth) return true;
  }
  return false;
}

function toggleFold(i: number) {
  const s = new Set(folded.value);
  if (s.has(i)) s.delete(i);
  else s.add(i);
  folded.value = s;
}

/** 折叠块内的行数（用于占位提示） */
function foldedCount(f: number): number {
  let n = 0;
  for (let j = f + 1; j < lines.value.length && lines.value[j].depth > lines.value[f].depth; j++) {
    n++;
  }
  return n;
}

/* ── 树形引导线：每层缩进一条竖线，层级结束处截断，最后子节点拐角（文件树风格） ── */

type TreeMark = "through" | "cut" | "branch-through" | "branch-cut";

/** 预计算：行 i 之后是否存在 depth >= d 的行（后缀，用于判断竖线是否延续） */
const suffix = computed<boolean[][]>(() => {
  const n = lines.value.length;
  const maxD = lines.value.reduce((m, l) => Math.max(m, l.depth), 0);
  const suf: boolean[][] = Array.from({ length: n + 1 }, () => Array(maxD + 1).fill(false));
  for (let i = n - 1; i >= 0; i--) {
    for (let d = 1; d <= maxD; d++) {
      suf[i][d] = lines.value[i].depth >= d || suf[i + 1][d];
    }
  }
  return suf;
});

/** 行 i 的树线标记（长度 = depth；每层一个） */
function treeMarks(i: number): TreeMark[] {
  const d = lines.value[i].depth;
  const marks: TreeMark[] = [];
  for (let lv = 1; lv <= d; lv++) {
    const hasBelow = suffix.value[i + 1]?.[lv] ?? false;
    if (lv === d) {
      marks.push(hasBelow ? "branch-through" : "branch-cut");
    } else {
      marks.push(hasBelow ? "through" : "cut");
    }
  }
  return marks;
}

/** 渲染计划：可见行 + 折叠占位（保持原始行号） */
const renderPlan = computed<{ i: number; placeholder?: number }[]>(() => {
  const out: { i: number; placeholder?: number }[] = [];
  for (let i = 0; i < lines.value.length; i++) {
    if (isHidden(i)) continue;
    out.push({ i });
    if (folded.value.has(i) && hasChildren(i)) {
      out.push({ i, placeholder: foldedCount(i) });
    }
  }
  return out;
});

const scroller = ref<HTMLElement | null>(null);
const gutter = ref<HTMLDivElement | null>(null);

function syncScroll() {
  if (gutter.value && scroller.value) gutter.value.scrollTop = scroller.value.scrollTop;
}
</script>

<template>
  <div
    class="flex min-h-0 w-full overflow-hidden rounded-md border border-border bg-surface-muted dark:border-border-dark dark:bg-surface-muted-dark"
  >
    <!-- 行号列（超高裁剪 + scrollTop 同步，不撑高容器） -->
    <div
      ref="gutter"
      class="w-[44px] shrink-0 select-none self-stretch overflow-hidden border-r border-border/60 bg-transparent py-[13px] pr-[10px] text-right font-mono text-body leading-relaxed text-text-muted/50 dark:border-border-dark/60 dark:text-text-muted-dark/50"
      aria-hidden="true"
    >
      <div v-for="p in renderPlan" :key="p.i" class="h-[21px] leading-[21px]">
        {{ p.placeholder !== undefined ? "" : p.i + 1 }}
      </div>
    </div>

    <!-- 内容区（内部滚动） -->
    <div
      ref="scroller"
      class="min-h-0 flex-1 cursor-text overflow-auto py-[13px] font-mono text-body leading-relaxed"
      @scroll="syncScroll"
    >
      <template v-for="p in renderPlan" :key="p.i">
        <!-- 普通行：树形引导线 + 折叠箭头（块起始行）+ 高亮内容 -->
        <div v-if="p.placeholder === undefined" class="flex h-[21px] items-center whitespace-pre">
          <!-- 每层缩进一格（2ch），画树线：through=贯穿竖线 cut=截断（层级结束） branch=拐角+横线 -->
          <span
            v-for="(m, mi) in treeMarks(p.i)"
            :key="mi"
            class="relative h-[21px] w-[2ch] shrink-0"
          >
            <span
              v-if="m === 'through' || m === 'branch-through'"
              class="tree-line absolute inset-y-0 left-0"
            />
            <span
              v-else-if="m === 'cut' || m === 'branch-cut'"
              class="tree-line absolute left-0 top-0 h-1/2"
            />
            <span
              v-if="m === 'branch-through' || m === 'branch-cut'"
              class="tree-line-h absolute left-0 top-1/2 w-[2ch]"
            />
          </span>
          <button
            v-if="hasChildren(p.i)"
            class="grid h-[21px] w-[18px] shrink-0 place-items-center text-text-muted transition-colors hover:text-tertiary-strong dark:text-text-muted-dark dark:hover:text-tertiary-dark"
            :title="folded.has(p.i) ? '展开节点' : '折叠节点'"
            @click="toggleFold(p.i)"
          >
            <svg
              width="10"
              height="10"
              viewBox="0 0 10 10"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <path v-if="!folded.has(p.i)" d="M2 2l4 3-4 3" />
              <path v-else d="M2 2l4 3-4 3" class="rotate-90 origin-center" />
            </svg>
          </button>
          <span v-else class="w-[18px] shrink-0" />
          <span class="min-w-0" v-html="lines[p.i].html" />
        </div>
        <!-- 折叠占位 -->
        <button
          v-else
          class="flex h-[21px] w-full cursor-pointer items-center gap-[8px] px-[18px] text-body-sm text-text-muted transition-colors hover:text-tertiary-strong dark:text-text-muted-dark dark:hover:text-tertiary-dark"
          :title="`展开 ${p.placeholder} 行`"
          @click="toggleFold(p.i)"
        >
          <span class="font-mono">⋯</span>
          <span>{{ p.placeholder }} 行已折叠</span>
        </button>
      </template>
    </div>
  </div>
</template>
